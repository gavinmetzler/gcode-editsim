//! The settings file: what the app remembers between sessions (M4.1).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use app_backend::files::MachineSource;
use app_backend::settings::{self, RememberedMachine, Settings};

/// A new empty folder for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "editsim-settings-{}-{name}-{n}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn user(file: &str) -> RememberedMachine {
    RememberedMachine::User {
        file: file.to_owned(),
    }
}

fn shipped(id: &str) -> RememberedMachine {
    RememberedMachine::Shipped { id: id.to_owned() }
}

#[test]
fn settings_round_trip_through_the_file() {
    let dir = TempDir::new("roundtrip");
    let path = dir.path("settings.json");
    for machine in [user("router.toml"), shipped("generic_3axis_mill")] {
        let s = Settings::new(Some(machine), Some("osai".to_owned()));
        settings::save(&path, &s).expect("save");
        let loaded = settings::load(&path);
        assert_eq!(loaded.problem, None);
        assert_eq!(loaded.settings, s);
    }
}

#[test]
fn the_file_is_small_readable_json() {
    let s = Settings::new(Some(user("router.toml")), Some("fanuc".to_owned()));
    let value: serde_json::Value = serde_json::from_str(&s.to_json()).expect("json");
    assert_eq!(
        value,
        serde_json::json!({
            "version": 1,
            "machine": { "kind": "user", "file": "router.toml" },
            "dialect": "fanuc",
        })
    );
}

#[test]
fn a_missing_file_gives_the_defaults_quietly() {
    let dir = TempDir::new("missing");
    let loaded = settings::load(&dir.path("settings.json"));
    assert_eq!(loaded.settings, Settings::default());
    assert_eq!(loaded.problem, None, "first start is not a problem");
}

#[test]
fn a_damaged_file_gives_the_defaults_and_says_why() {
    let dir = TempDir::new("damaged");
    let path = dir.path("settings.json");
    for bad in [&b"{ not json"[..], b"", b"[1, 2]", b"\"text\"", b"\xff\xfe"] {
        fs::write(&path, bad).expect("write");
        let loaded = settings::load(&path);
        assert_eq!(loaded.settings, Settings::default(), "{bad:?}");
        let problem = loaded.problem.expect("a problem is reported");
        assert!(problem.contains("settings.json"), "{problem}");
    }
}

#[test]
fn a_huge_file_is_not_read() {
    let dir = TempDir::new("huge");
    let path = dir.path("settings.json");
    let mut text = b"{\"dialect\":\"fanuc\",\"x\":\"".to_vec();
    text.resize(settings::MAX_SETTINGS_BYTES as usize + 10, b'a');
    text.extend_from_slice(b"\"}");
    fs::write(&path, text).expect("write");
    let loaded = settings::load(&path);
    assert_eq!(loaded.settings, Settings::default());
    assert!(loaded.problem.is_some());
}

#[test]
fn a_bad_setting_is_dropped_and_the_rest_kept() {
    let s = Settings::from_json(br#"{"machine": 5, "dialect": "osai"}"#);
    assert_eq!(s.settings.machine, None);
    assert_eq!(s.settings.dialect.as_deref(), Some("osai"));
    assert!(s.problem.expect("reported").contains("machine"));

    let s = Settings::from_json(br#"{"machine": {"kind":"user","file":"a.toml"}, "dialect": 7}"#);
    assert_eq!(s.settings.machine, Some(user("a.toml")));
    assert_eq!(s.settings.dialect, None);
    assert!(s.problem.expect("reported").contains("dialect"));

    let s = Settings::from_json(br#"{"machine": {"kind":"cloud","id":"x"}}"#);
    assert_eq!(s.settings.machine, None);
}

#[test]
fn settings_this_version_does_not_know_are_kept_when_saving() {
    // A later version (M4.2's window layout, say) wrote more than this one
    // knows; saving the machine must not lose it.
    let dir = TempDir::new("unknown");
    let path = dir.path("settings.json");
    fs::write(
        &path,
        br#"{"version": 1, "dialect": "fanuc", "layout": {"editor": 0.4}}"#,
    )
    .expect("write");
    settings::update(&path, |s| s.machine = Some(user("router.toml"))).expect("update");
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("read")).expect("json");
    assert_eq!(value["layout"]["editor"], 0.4);
    assert_eq!(value["dialect"], "fanuc");
    assert_eq!(value["machine"]["file"], "router.toml");
}

#[test]
fn update_starts_from_the_defaults_when_the_file_is_missing_or_damaged() {
    let dir = TempDir::new("update");
    let path = dir.path("config/settings.json");
    // The folder is created if needed.
    settings::update(&path, |s| s.dialect = Some("osai".to_owned())).expect("update");
    assert_eq!(
        settings::load(&path).settings.dialect.as_deref(),
        Some("osai")
    );
    fs::write(&path, b"{ damaged").expect("write");
    settings::update(&path, |s| s.dialect = Some("fanuc".to_owned())).expect("update");
    let loaded = settings::load(&path);
    assert_eq!(loaded.problem, None);
    assert_eq!(loaded.settings.dialect.as_deref(), Some("fanuc"));
}

#[test]
fn a_user_machine_is_remembered_by_file_name_in_the_machine_folder() {
    let dir = Path::new("/home/me/.config/app/machines");
    let source = MachineSource::User(dir.join("router.toml"));
    let remembered = RememberedMachine::from_source(&source, dir).expect("in the folder");
    assert_eq!(remembered, user("router.toml"));
    assert_eq!(remembered.to_source(dir), Some(source));

    // A file elsewhere is not one of the user's machine files.
    let elsewhere = MachineSource::User(PathBuf::from("/tmp/router.toml"));
    assert_eq!(RememberedMachine::from_source(&elsewhere, dir), None);
    let nested = MachineSource::User(dir.join("old/router.toml"));
    assert_eq!(RememberedMachine::from_source(&nested, dir), None);
}

#[test]
fn a_shipped_machine_is_remembered_by_id() {
    let dir = Path::new("/m");
    let source = MachineSource::Shipped("generic_3axis_router");
    let remembered = RememberedMachine::from_source(&source, dir).expect("shipped");
    assert_eq!(remembered, shipped("generic_3axis_router"));
    assert_eq!(remembered.to_source(dir), Some(source));
    // An example this version does not ship any more.
    assert_eq!(shipped("generic_5axis").to_source(dir), None);
}

#[test]
fn a_remembered_file_name_cannot_leave_the_machine_folder() {
    // The settings file is user-editable; a crafted name must never make
    // the app read a file outside the machine folder.
    let dir = Path::new("/m");
    for bad in [
        "../secret.toml",
        "..",
        ".",
        "",
        "/etc/passwd",
        "sub/a.toml",
        "sub\\a.toml",
        "a.txt",
    ] {
        assert_eq!(user(bad).to_source(dir), None, "{bad:?}");
    }
    assert_eq!(
        user("Fräse 2.TOML").to_source(dir),
        Some(MachineSource::User(dir.join("Fräse 2.TOML")))
    );
}

#[test]
fn a_remembered_machine_has_a_name_to_show() {
    assert_eq!(user("router.toml").label(), "router.toml");
    assert_eq!(shipped("generic_3axis_mill").label(), "generic_3axis_mill");
}
