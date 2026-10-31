//! The window's app id must match the desktop file's name, or Linux
//! desktops cannot link the window to its launcher and show a generic
//! icon (the Wayland logo on KDE Plasma) instead of the app's.

/// The `identifier` in `tauri.conf.json`.
fn identifier() -> String {
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
    conf["identifier"].as_str().expect("identifier").to_owned()
}

#[cfg(target_os = "linux")]
#[test]
fn the_window_reports_the_identifier_as_its_app_id() {
    let id = identifier();
    gcode_editsim::set_window_app_id(&id);
    // GTK takes the Wayland app id and the X11 WM_CLASS from this name.
    assert_eq!(glib::prgname().as_deref(), Some(id.as_str()));
}

#[test]
fn the_flatpak_files_are_named_for_the_identifier() {
    let id = identifier();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging");
    let desktop = std::fs::read_to_string(root.join(format!("flatpak/{id}.desktop")))
        .expect("desktop file named for the identifier");
    assert!(
        desktop.lines().any(|l| l == format!("Icon={id}")),
        "{desktop}"
    );
    assert!(
        desktop.lines().any(|l| l == format!("StartupWMClass={id}")),
        "{desktop}"
    );
    assert!(root.join(format!("icons/{id}.svg")).is_file());
    let manifest =
        std::fs::read_to_string(root.join(format!("flatpak/{id}.yml"))).expect("manifest");
    assert!(
        manifest.lines().any(|l| l == format!("app-id: {id}")),
        "{manifest}"
    );
}
