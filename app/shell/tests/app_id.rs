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

/// Width and height of a PNG file, from its header.
fn png_size(path: &std::path::Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        &bytes[..8],
        b"\x89PNG\r\n\x1a\n",
        "{} is a PNG",
        path.display()
    );
    let be =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    (be(16), be(20))
}

#[test]
fn the_app_icons_exist_in_the_sizes_tauri_and_desktops_use() {
    // M4.10: the icon is drawn once (packaging/icons/<id>.svg) and the PNGs
    // Tauri embeds (the window icon) are rendered from it.
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
    let icons: Vec<&str> = conf["bundle"]["icon"]
        .as_array()
        .expect("bundle.icon")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    let shell = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sizes = Vec::new();
    for icon in &icons {
        let (w, h) = png_size(&shell.join(icon));
        assert_eq!(w, h, "{icon} is square");
        sizes.push(w);
    }
    sizes.sort_unstable();
    assert_eq!(sizes, vec![32, 128, 256, 512], "{icons:?}");

    let id = identifier();
    let svg = std::fs::read_to_string(shell.join(format!("../../packaging/icons/{id}.svg")))
        .expect("svg");
    assert!(
        !svg.to_ascii_lowercase().contains("placeholder"),
        "the real icon, not the placeholder"
    );
    assert!(
        svg.contains("viewBox=\"0 0 128 128\""),
        "drawn on a 128-unit grid"
    );
}
