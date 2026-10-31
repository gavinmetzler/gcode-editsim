//! GCode EditSim desktop app: a thin Tauri shell over `app-backend`.
//!
//! Every command forwards to `app_backend`; no G-code logic lives here.
//! Bulk data never crosses the boundary as JSON (ARCHITECTURE.md §8):
//!
//! - `interpret` takes the program as the raw request body and returns
//!   the render buffer as raw bytes.
//! - `load_file` returns the file's raw bytes; `save_file` takes them as
//!   the raw body, with the path in the `x-path` header (percent-encoded
//!   UTF-8, as `encodeURIComponent` produces).
//!
//! The window shows the UI from `ui/` (built into `ui/dist`, embedded at
//! compile time: build the UI before building this crate).
//!
//! File paths come from the native dialogs (`tauri-plugin-dialog`), so the
//! UI knows the real path of the open file and can save over it.
//!
//! Small settings (the machine, the dialect, options) are JSON. The
//! machine and dialect are chosen once with `select_machine` and
//! `set_run_options` and kept in [`AppState`], so each `interpret` call
//! carries only the program. Interpretation and file reads run on a
//! worker thread, never on the UI thread (§8.5).

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use app_backend::files::{self, MachineSource};
use app_backend::{BackendError, RunOptions};
use serde::{Deserialize, Serialize};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{Manager, Runtime, State};

/// What the next run uses.
#[derive(Debug, Clone)]
struct Settings {
    /// Text of the selected machine file, if one has been selected.
    machine_toml: Option<String>,
    dialect: String,
    options: RunOptions,
}

/// The app's state, shared by all commands.
#[derive(Debug)]
pub struct AppState {
    settings: Mutex<Settings>,
    machine_dir: PathBuf,
}

impl AppState {
    /// Starts with the Fanuc dialect, default options, no machine selected,
    /// and `machine_dir` as the user's machine folder.
    pub fn new(machine_dir: PathBuf) -> Self {
        Self {
            settings: Mutex::new(Settings {
                machine_toml: None,
                dialect: "fanuc".to_owned(),
                options: RunOptions::default(),
            }),
            machine_dir,
        }
    }

    fn settings(&self) -> Settings {
        // A panic while holding the lock cannot leave Settings half
        // updated (every update is a single assignment), so a poisoned
        // lock is still safe to use.
        match self.settings.lock() {
            Ok(s) => s.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn update(&self, f: impl FnOnce(&mut Settings)) {
        match self.settings.lock() {
            Ok(mut s) => f(&mut s),
            Err(poisoned) => f(&mut poisoned.into_inner()),
        }
    }
}

/// An error as the UI sees it: a message to show.
fn message(e: BackendError) -> String {
    e.to_string()
}

/// A dialect the user can choose.
#[derive(Debug, Serialize)]
pub struct DialectEntry {
    id: &'static str,
    name: &'static str,
    stub: bool,
}

/// The dialects, in the order to offer them.
#[tauri::command]
fn list_dialects() -> Vec<DialectEntry> {
    app_backend::dialects()
        .iter()
        .map(|d| DialectEntry {
            id: d.id,
            name: d.name,
            stub: d.stub,
        })
        .collect()
}

/// Which machine file: a shipped example by id, or a user file by path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "lowercase")]
pub enum MachineRef {
    /// A shipped example.
    Shipped(String),
    /// A file in the user's machine folder.
    User(String),
}

/// A machine file the user can choose.
#[derive(Debug, Serialize)]
pub struct MachineEntry {
    label: String,
    source: MachineRef,
    problem: Option<String>,
}

/// The machine files in the user's machine folder, or the shipped examples
/// when it has none.
#[tauri::command]
async fn list_machines(state: State<'_, AppState>) -> Result<Vec<MachineEntry>, String> {
    let dir = state.machine_dir.clone();
    let list = tauri::async_runtime::spawn_blocking(move || files::list_machine_files(&dir))
        .await
        .map_err(|e| e.to_string())?
        .map_err(message)?;
    Ok(list
        .into_iter()
        .map(|m| MachineEntry {
            label: m.label,
            source: match m.source {
                MachineSource::Shipped(id) => MachineRef::Shipped(id.to_owned()),
                MachineSource::User(path) => MachineRef::User(path.display().to_string()),
            },
            problem: m.problem,
        })
        .collect())
}

/// The user's machine folder, created if missing, so the UI can tell the
/// user where to put machine files.
#[tauri::command]
fn machine_folder(state: State<'_, AppState>) -> Result<String, String> {
    std::fs::create_dir_all(&state.machine_dir).map_err(|e| {
        message(BackendError::Io {
            path: state.machine_dir.display().to_string(),
            source: e,
        })
    })?;
    Ok(state.machine_dir.display().to_string())
}

/// Selects the machine for later runs. Returns its name.
#[tauri::command]
fn select_machine(source: MachineRef, state: State<'_, AppState>) -> Result<String, String> {
    let source = match source {
        MachineRef::Shipped(id) => files::SHIPPED_MACHINES
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(i, _)| MachineSource::Shipped(i))
            .ok_or_else(|| format!("no shipped machine {id:?}"))?,
        MachineRef::User(path) => MachineSource::User(PathBuf::from(path)),
    };
    let text = files::read_machine_file(&source).map_err(message)?;
    let name = app_backend::load_machine(&text)
        .map_err(message)?
        .name()
        .to_owned();
    state.update(|s| s.machine_toml = Some(text));
    Ok(name)
}

/// Sets the dialect and run options for later runs. `tolerance` is the
/// arc tolerance in mm; leave it out for the default.
#[tauri::command]
fn set_run_options(
    dialect: String,
    block_delete: bool,
    tolerance: Option<f64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    app_backend::load_dialect(&dialect).map_err(message)?;
    let tolerance = tolerance.unwrap_or(RunOptions::default().tolerance);
    if !(tolerance > 0.0 && tolerance.is_finite()) {
        return Err(message(BackendError::InvalidTolerance));
    }
    state.update(|s| {
        s.dialect = dialect;
        s.options = RunOptions {
            block_delete,
            tolerance,
        };
    });
    Ok(())
}

/// The raw body of a request, or an error naming what expected it.
fn raw_body(request: &Request<'_>, what: &str) -> Result<Vec<u8>, String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(_) => Err(format!("{what}: send the contents as raw bytes")),
    }
}

/// A percent-encoded header value, decoded.
fn header(request: &Request<'_>, name: &str) -> Result<Option<String>, String> {
    match request.headers().get(name) {
        None => Ok(None),
        Some(value) => value
            .to_str()
            .ok()
            .and_then(percent_decode)
            .map(Some)
            .ok_or_else(|| format!("header {name} is not valid percent-encoded UTF-8")),
    }
}

/// Decodes `%XX` escapes (as `encodeURIComponent` writes them). `None` if
/// an escape is malformed or the result is not UTF-8.
pub fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Interprets the program in the raw request body with the selected
/// machine, dialect and options, and returns the render buffer. The
/// `x-file-name` header (percent-encoded) names the program in
/// diagnostics.
#[tauri::command]
async fn interpret(request: Request<'_>, state: State<'_, AppState>) -> Result<Response, String> {
    let program = raw_body(&request, "interpret")?;
    let file_name = header(&request, "x-file-name")?.unwrap_or_else(|| "program".to_owned());
    let settings = state.settings();
    let machine = settings
        .machine_toml
        .ok_or_else(|| "select a machine first".to_owned())?;
    let buffer = tauri::async_runtime::spawn_blocking(move || {
        app_backend::interpret_to_buffer(
            &program,
            &file_name,
            &machine,
            &settings.dialect,
            settings.options,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(message)?;
    Ok(Response::new(buffer))
}

/// Reads a program file and returns its raw bytes.
#[tauri::command]
async fn load_file(path: String) -> Result<Response, String> {
    let bytes = tauri::async_runtime::spawn_blocking(move || files::read_program(Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(message)?;
    Ok(Response::new(bytes))
}

/// Saves the raw request body to the path in the `x-path` header.
#[tauri::command]
async fn save_file(request: Request<'_>) -> Result<(), String> {
    let contents = raw_body(&request, "save_file")?;
    let path =
        header(&request, "x-path")?.ok_or_else(|| "save_file: no x-path header".to_owned())?;
    tauri::async_runtime::spawn_blocking(move || files::save_program(Path::new(&path), &contents))
        .await
        .map_err(|e| e.to_string())?
        .map_err(message)
}

/// Adds the app's commands to `builder`. The caller manages an
/// [`AppState`] (see [`run`]; tests use their own folder).
pub fn with_commands<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        list_dialects,
        list_machines,
        machine_folder,
        select_machine,
        set_run_options,
        interpret,
        load_file,
        save_file,
    ])
}

/// Names the program `id` for the desktop, before any window opens. On
/// Linux, GTK reports this name as the window's Wayland app id and X11
/// class; it must match the desktop file's name (`<id>.desktop`) for the
/// desktop to show the app's icon. By default it is the executable's name
/// (`gcode-editsim`), which matches nothing. Elsewhere it does nothing.
///
/// Tauri's own `enableGTKAppId` would also do this, but registers the app
/// on the session bus, which allows only one copy of the app to run.
pub fn set_window_app_id(id: &str) {
    #[cfg(target_os = "linux")]
    glib::set_prgname(Some(id));
    #[cfg(not(target_os = "linux"))]
    let _ = id;
}

/// Runs the desktop app. The user's machine folder is `machines` in the
/// app's configuration folder; on start it gets the shipped examples if it
/// has no machine files yet.
pub fn run() -> Result<(), tauri::Error> {
    let context = tauri::generate_context!();
    set_window_app_id(&context.config().identifier);
    with_commands(tauri::Builder::default())
        // Native open, save and message dialogs (the UI picks paths with
        // them; files are still read and written by this app's commands).
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?.join("machines");
            // Not fatal: the shipped examples are listed when the folder
            // has no files, so the app still has machines to choose.
            if let Err(e) = files::seed_machine_folder(&dir) {
                eprintln!("could not set up the machine folder: {e}");
            }
            app.manage(AppState::new(dir));
            Ok(())
        })
        .run(context)
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn decodes_what_encode_uri_component_writes() {
        assert_eq!(
            percent_decode("%2Fhome%2Fme%2FTeil%20%C3%BC.nc").as_deref(),
            Some("/home/me/Teil ü.nc")
        );
        assert_eq!(percent_decode("plain.nc").as_deref(), Some("plain.nc"));
        assert_eq!(percent_decode("%41%62").as_deref(), Some("Ab"));
    }

    #[test]
    fn rejects_bad_escapes_and_bad_utf8() {
        for bad in ["%", "%4", "%zz", "%C3", "%FF", "%%41"] {
            assert_eq!(percent_decode(bad), None, "{bad}");
        }
    }
}
