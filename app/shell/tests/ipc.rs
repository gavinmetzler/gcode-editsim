//! The commands through Tauri's IPC layer, on Tauri's mock runtime (no
//! window or WebKit needed at run time). Checks that bulk data crosses as
//! raw bytes in both directions, never JSON.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use gcode_editsim::{with_commands, AppState};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{
    get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY,
};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};

/// A new empty folder for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("editsim-shell-{}-{name}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn window(machine_dir: PathBuf) -> WebviewWindow<MockRuntime> {
    let settings = machine_dir.with_file_name("settings.json");
    window_with(machine_dir, settings)
}

fn window_with(machine_dir: PathBuf, settings_file: PathBuf) -> WebviewWindow<MockRuntime> {
    let app = with_commands(mock_builder())
        .manage(AppState::new(machine_dir, settings_file))
        .build(mock_context(noop_assets()))
        .expect("app builds");
    WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("window builds")
}

fn call(
    w: &WebviewWindow<MockRuntime>,
    cmd: &str,
    body: InvokeBody,
    headers: &[(&'static str, &str)],
) -> Result<InvokeResponseBody, serde_json::Value> {
    // The app's own origin on Linux and macOS.
    call_from(w, "tauri://localhost", cmd, body, headers)
}

fn call_from(
    w: &WebviewWindow<MockRuntime>,
    origin: &str,
    cmd: &str,
    body: InvokeBody,
    headers: &[(&'static str, &str)],
) -> Result<InvokeResponseBody, serde_json::Value> {
    let mut map = tauri::http::HeaderMap::new();
    for (k, v) in headers {
        map.insert(*k, v.parse().expect("header value"));
    }
    get_ipc_response(
        w,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: origin.parse().expect("url"),
            body,
            headers: map,
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
}

fn json(
    w: &WebviewWindow<MockRuntime>,
    cmd: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    match call(w, cmd, InvokeBody::Json(args), &[])? {
        InvokeResponseBody::Json(text) => Ok(serde_json::from_str(&text).expect("json")),
        InvokeResponseBody::Raw(_) => panic!("{cmd}: expected JSON"),
    }
}

fn raw(response: Result<InvokeResponseBody, serde_json::Value>) -> Vec<u8> {
    match response.expect("ok") {
        InvokeResponseBody::Raw(bytes) => bytes,
        InvokeResponseBody::Json(text) => panic!("expected raw bytes, got JSON {text}"),
    }
}

fn select_mill(w: &WebviewWindow<MockRuntime>) {
    let name = json(
        w,
        "select_machine",
        serde_json::json!({ "source": { "kind": "shipped", "id": "generic_3axis_mill" } }),
    )
    .expect("selects");
    assert_eq!(name, "Generic 3-axis VMC");
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[test]
fn dialects_are_listed() {
    let dir = TempDir::new("dialects");
    let w = window(dir.0.clone());
    let list = json(&w, "list_dialects", serde_json::json!({})).expect("ok");
    assert_eq!(list[0]["id"], "fanuc");
    assert_eq!(list[1]["id"], "osai");
    assert_eq!(list[1]["stub"], true);
}

#[test]
fn interpret_returns_the_render_buffer_as_raw_bytes() {
    let dir = TempDir::new("interpret");
    let w = window(dir.0.clone());
    select_mill(&w);
    let program = b"G21 G90 G54 G0 X0. Y0. Z5.\nG1 Z-1. F100.\nM30\n".to_vec();
    let b = raw(call(
        &w,
        "interpret",
        InvokeBody::Raw(program),
        &[("x-file-name", "part%20one.nc")],
    ));
    assert_eq!(&b[0..4], b"RPCG");
    assert_eq!(u32_at(&b, 12), 3, "rapid, linear, program end");
}

#[test]
fn interpret_needs_a_machine_and_a_raw_body() {
    let dir = TempDir::new("needs");
    let w = window(dir.0.clone());
    let err =
        call(&w, "interpret", InvokeBody::Raw(b"G0 X1.\n".to_vec()), &[]).expect_err("no machine");
    assert!(err.to_string().contains("select a machine"), "{err}");
    select_mill(&w);
    let err = call(
        &w,
        "interpret",
        InvokeBody::Json(serde_json::json!({ "program": "G0 X1." })),
        &[],
    )
    .expect_err("JSON body");
    assert!(err.to_string().contains("raw bytes"), "{err}");
}

#[test]
fn run_options_choose_the_dialect() {
    let dir = TempDir::new("options");
    let w = window(dir.0.clone());
    select_mill(&w);
    json(
        &w,
        "set_run_options",
        serde_json::json!({ "dialect": "osai", "blockDelete": false, "tolerance": 0.05 }),
    )
    .expect("set");
    // (UAO,1) is a command on Osai; on Fanuc it would be a comment.
    let b = raw(call(
        &w,
        "interpret",
        InvokeBody::Raw(b"G71\n(UAO,1)\nX10 Y20\n".to_vec()),
        &[],
    ));
    assert_eq!(u32_at(&b, 12), 1);
    for bad in [
        serde_json::json!({ "dialect": "haas", "blockDelete": false }),
        serde_json::json!({ "dialect": "fanuc", "blockDelete": false, "tolerance": 0.0 }),
    ] {
        assert!(json(&w, "set_run_options", bad).is_err());
    }
}

#[test]
fn files_round_trip_as_raw_bytes_with_unicode_paths() {
    let dir = TempDir::new("files");
    let w = window(dir.0.clone());
    let path = dir.0.join("Teil ü 1.nc");
    let encoded: String = path
        .display()
        .to_string()
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let contents = b"G0 X1.\n\xff not UTF-8 \n".to_vec();
    match call(
        &w,
        "save_file",
        InvokeBody::Raw(contents.clone()),
        &[("x-path", &encoded)],
    )
    .expect("saved")
    {
        InvokeResponseBody::Json(text) => assert_eq!(text, "null"),
        InvokeResponseBody::Raw(_) => panic!("save returns nothing"),
    }
    let loaded = raw(call(
        &w,
        "load_file",
        InvokeBody::Json(serde_json::json!({ "path": path.display().to_string() })),
        &[],
    ));
    assert_eq!(loaded, contents);
}

#[test]
fn bad_save_requests_are_errors() {
    let dir = TempDir::new("badsave");
    let w = window(dir.0.clone());
    assert!(
        call(&w, "save_file", InvokeBody::Raw(vec![1]), &[]).is_err(),
        "no path"
    );
    assert!(call(
        &w,
        "save_file",
        InvokeBody::Raw(vec![1]),
        &[("x-path", "%zz")]
    )
    .is_err());
}

#[test]
fn user_machine_files_are_listed_and_selectable() {
    let dir = TempDir::new("machines");
    let w = window(dir.0.join("machines"));
    let folder = json(&w, "machine_folder", serde_json::json!({})).expect("folder");
    assert!(dir.0.join("machines").is_dir(), "created: {folder}");
    std::fs::write(dir.0.join("machines/lathe.toml"), machine_lathe_text()).expect("write");
    let list = json(&w, "list_machines", serde_json::json!({})).expect("list");
    // Once the folder has machine files, only they are listed.
    let entries = list.as_array().expect("a list");
    assert_eq!(entries.len(), 1);
    let user = &entries[0];
    assert_eq!(user["label"], "Generic 2-axis lathe");
    assert_eq!(user["source"]["kind"], "user");
    let name = json(
        &w,
        "select_machine",
        serde_json::json!({ "source": user["source"] }),
    )
    .expect("select");
    assert_eq!(name, "Generic 2-axis lathe");
}

fn machine_lathe_text() -> String {
    // The shipped lathe example, fetched through the app itself.
    app_backend::files::shipped_machine("generic_2axis_lathe")
        .expect("shipped")
        .to_owned()
}

#[test]
fn a_remote_page_cannot_call_the_commands() {
    let dir = TempDir::new("remote");
    let w = window(dir.0.clone());
    let err = call_from(
        &w,
        "https://example.com",
        "list_dialects",
        InvokeBody::Json(serde_json::json!({})),
        &[],
    )
    .expect_err("refused");
    assert!(err.to_string().contains("not allowed"), "{err}");
}

/// A config folder with a machine folder holding the router example, and
/// the paths a window uses there.
fn config_with_router(name: &str) -> (TempDir, PathBuf, PathBuf) {
    let dir = TempDir::new(name);
    let machines = dir.0.join("machines");
    std::fs::create_dir_all(&machines).expect("machines");
    std::fs::write(
        machines.join("router.toml"),
        app_backend::files::shipped_machine("generic_3axis_router").expect("shipped"),
    )
    .expect("write");
    let settings = dir.0.join("settings.json");
    (dir, machines, settings)
}

#[test]
fn nothing_is_remembered_on_the_first_start() {
    let (_dir, machines, settings) = config_with_router("first");
    let w = window_with(machines, settings.clone());
    let r = json(&w, "remembered_settings", serde_json::json!({})).expect("ok");
    assert_eq!(
        r,
        serde_json::json!({ "machine": null, "dialect": null, "layout": null, "problem": null })
    );
    assert!(!settings.exists(), "reading does not create the file");
}

#[test]
fn the_choice_is_remembered_for_the_next_session() {
    let (_dir, machines, settings) = config_with_router("remember");
    let router = machines.join("router.toml").display().to_string();
    {
        let w = window_with(machines.clone(), settings.clone());
        // The UI passes back the source exactly as list_machines gave it.
        let list = json(&w, "list_machines", serde_json::json!({})).expect("list");
        assert_eq!(list[0]["source"]["id"], router.as_str());
        json(
            &w,
            "remember_choice",
            serde_json::json!({ "machine": list[0]["source"], "dialect": "osai" }),
        )
        .expect("remembered");
    }
    // A new session: a new app over the same folders.
    let w = window_with(machines, settings.clone());
    let r = json(&w, "remembered_settings", serde_json::json!({})).expect("ok");
    assert_eq!(
        r["machine"],
        serde_json::json!({ "kind": "user", "id": router })
    );
    assert_eq!(r["dialect"], "osai");
    assert_eq!(r["problem"], serde_json::Value::Null);
    // Kept by file name only.
    let text = std::fs::read_to_string(&settings).expect("saved");
    assert!(text.contains("\"router.toml\""), "{text}");
    assert!(!text.contains("machines/router.toml"), "{text}");
}

#[test]
fn a_shipped_machine_is_remembered_too() {
    let (_dir, machines, settings) = config_with_router("shipped");
    let w = window_with(machines, settings);
    json(
        &w,
        "remember_choice",
        serde_json::json!({
            "machine": { "kind": "shipped", "id": "generic_3axis_mill" },
            "dialect": "fanuc"
        }),
    )
    .expect("remembered");
    let r = json(&w, "remembered_settings", serde_json::json!({})).expect("ok");
    assert_eq!(
        r["machine"],
        serde_json::json!({ "kind": "shipped", "id": "generic_3axis_mill" })
    );
}

#[test]
fn a_damaged_settings_file_is_reported_not_an_error() {
    let (_dir, machines, settings) = config_with_router("damaged");
    std::fs::write(&settings, b"{ damaged").expect("write");
    let w = window_with(machines, settings);
    let r = json(&w, "remembered_settings", serde_json::json!({})).expect("never an error");
    assert_eq!(r["machine"], serde_json::Value::Null);
    assert_eq!(r["dialect"], serde_json::Value::Null);
    let problem = r["problem"].as_str().expect("a problem");
    assert!(problem.contains("settings.json"), "{problem}");
}

#[test]
fn a_crafted_file_name_is_not_followed_out_of_the_machine_folder() {
    let (_dir, machines, settings) = config_with_router("crafted");
    std::fs::write(
        &settings,
        br#"{"machine": {"kind": "user", "file": "../secret.toml"}, "dialect": "fanuc"}"#,
    )
    .expect("write");
    let w = window_with(machines, settings);
    let r = json(&w, "remembered_settings", serde_json::json!({})).expect("ok");
    assert_eq!(r["machine"], serde_json::Value::Null);
    assert_eq!(r["dialect"], "fanuc");
    let problem = r["problem"].as_str().expect("a problem");
    assert!(problem.contains("../secret.toml"), "{problem}");
}

#[test]
fn only_real_choices_are_remembered() {
    let (_dir, machines, settings) = config_with_router("bad");
    let w = window_with(machines.clone(), settings.clone());
    for bad in [
        // Not in the machine folder.
        serde_json::json!({ "machine": { "kind": "user", "id": "/tmp/router.toml" }, "dialect": "fanuc" }),
        serde_json::json!({ "machine": { "kind": "shipped", "id": "nope" }, "dialect": "fanuc" }),
        serde_json::json!({
            "machine": { "kind": "user", "id": machines.join("router.toml").display().to_string() },
            "dialect": "haas"
        }),
    ] {
        assert!(json(&w, "remember_choice", bad.clone()).is_err(), "{bad}");
    }
    assert!(!settings.exists(), "nothing was written");
}

#[test]
fn the_pane_layout_is_remembered_for_the_next_session() {
    let (_dir, machines, settings) = config_with_router("layout");
    {
        let w = window_with(machines.clone(), settings.clone());
        let r = json(&w, "remembered_settings", serde_json::json!({})).expect("ok");
        assert_eq!(r["layout"], serde_json::Value::Null, "nothing yet");
        json(
            &w,
            "remember_layout",
            serde_json::json!({ "layout": { "editor": 0.3, "panel": 0.4, "maximised": "viewport" } }),
        )
        .expect("remembered");
    }
    let w = window_with(machines, settings);
    let r = json(&w, "remembered_settings", serde_json::json!({})).expect("ok");
    assert_eq!(
        r["layout"],
        serde_json::json!({ "editor": 0.3, "panel": 0.4, "maximised": "viewport" })
    );
}

#[test]
fn remembering_the_layout_keeps_the_machine_and_dialect() {
    let (_dir, machines, settings) = config_with_router("layout-keeps");
    let w = window_with(machines.clone(), settings);
    let router = machines.join("router.toml").display().to_string();
    json(
        &w,
        "remember_choice",
        serde_json::json!({ "machine": { "kind": "user", "id": router }, "dialect": "osai" }),
    )
    .expect("choice");
    json(
        &w,
        "remember_layout",
        serde_json::json!({ "layout": { "editor": 0.5 } }),
    )
    .expect("layout");
    let r = json(&w, "remembered_settings", serde_json::json!({})).expect("ok");
    assert_eq!(r["dialect"], "osai");
    assert_eq!(r["machine"]["id"], router.as_str());
    assert_eq!(r["layout"], serde_json::json!({ "editor": 0.5 }));
}

#[test]
fn a_layout_out_of_range_is_refused() {
    let (_dir, machines, settings) = config_with_router("layout-bad");
    let w = window_with(machines, settings.clone());
    for bad in [
        serde_json::json!({ "layout": { "editor": 0.0 } }),
        serde_json::json!({ "layout": { "panel": 1.2 } }),
        serde_json::json!({ "layout": { "editor": -1 } }),
    ] {
        assert!(json(&w, "remember_layout", bad.clone()).is_err(), "{bad}");
    }
    assert!(!settings.exists(), "nothing was written");
}

#[test]
fn reset_layout_forgets_the_layout_and_the_window_but_not_the_choice() {
    let (_dir, machines, settings) = config_with_router("reset");
    std::fs::write(
        &settings,
        br#"{"version":1,"dialect":"osai","layout":{"editor":0.3},"window":{"width":900,"height":600,"maximized":true}}"#,
    )
    .expect("write");
    let w = window_with(machines, settings.clone());
    json(&w, "reset_layout", serde_json::json!({})).expect("reset");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings).expect("read")).expect("json");
    assert_eq!(value["dialect"], "osai");
    assert_eq!(value.get("layout"), None);
    assert_eq!(value.get("window"), None);
}

#[test]
fn the_legend_and_diagnostics_filters_are_remembered() {
    let (_dir, machines, settings) = config_with_router("legend");
    let layout = serde_json::json!({
        "editor": 0.4,
        "legend_folded": true,
        "hidden_diagnostics": ["info"]
    });
    {
        let w = window_with(machines.clone(), settings.clone());
        json(
            &w,
            "remember_layout",
            serde_json::json!({ "layout": layout }),
        )
        .expect("saved");
    }
    let w = window_with(machines, settings);
    let r = json(&w, "remembered_settings", serde_json::json!({})).expect("ok");
    assert_eq!(r["layout"], layout);
}
