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

/// M5.2: the app chooses WebKitGTK's drawing setup itself and keeps any
/// variable that is already set, so a fixed setting in the manifest would
/// override its choice for everyone (as `WEBKIT_DISABLE_DMABUF_RENDERER=1`
/// did, making NVIDIA on Wayland copy every frame through the CPU).
#[test]
fn the_flatpak_leaves_the_webkit_drawing_setup_to_the_app() {
    let id = identifier();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging");
    let manifest =
        std::fs::read_to_string(root.join(format!("flatpak/{id}.yml"))).expect("manifest");
    for var in [
        "WEBKIT_DISABLE_DMABUF_RENDERER",
        "__NV_DISABLE_EXPLICIT_SYNC",
        "WEBKIT_USE_SKIA_FOR_COMPOSITION",
    ] {
        let set = manifest
            .lines()
            .any(|l| l.trim_start().starts_with("- --env=") && l.contains(var));
        assert!(!set, "the manifest sets {var}");
    }
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
    for icon in icons.iter().filter(|i| i.ends_with(".png")) {
        let (w, h) = png_size(&shell.join(icon));
        assert_eq!(w, h, "{icon} is square");
        sizes.push(w);
    }
    sizes.sort_unstable();
    assert_eq!(sizes, vec![32, 128, 256, 512], "{icons:?}");
    // M5.7: the Windows (.ico) and macOS (.icns) icons, rendered from the
    // same drawing (`cargo tauri icon`), are real files of their kind.
    for (icon, magic) in [
        ("icons/icon.ico", &b"\x00\x00\x01\x00"[..]),
        ("icons/icon.icns", &b"icns"[..]),
    ] {
        assert!(icons.contains(&icon), "{icon} is listed: {icons:?}");
        let bytes = std::fs::read(shell.join(icon)).unwrap_or_else(|e| panic!("{icon}: {e}"));
        assert!(bytes.starts_with(magic), "{icon} is that kind of file");
    }
    assert_eq!(icons.len(), 6, "only these icons: {icons:?}");

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
    // Image loaders (gdk-pixbuf, so the Flatpak's AppStream step) tell an
    // SVG by its start: `<svg` must come within the first 256 bytes. A long
    // comment before it broke the Flatpak build after M4.10.
    let at = svg.find("<svg").expect("an <svg> element");
    assert!(
        at < 256,
        "<svg starts at byte {at}; move comments inside the element"
    );
}

#[test]
fn g_code_files_are_a_type_the_app_opens_with_its_icon() {
    // M4.11. How desktops resolve the clash with NetCDF is checked by
    // packaging/flatpak/check-mime.sh (it needs gio, and PyQt6 for KDE).
    let id = identifier();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/flatpak");
    let mime = std::fs::read_to_string(root.join(format!("{id}.xml")))
        .expect("MIME package named for the identifier");
    let gcode = mime
        .split("<mime-type ")
        .find(|t| t.starts_with("type=\"text/x-cnc-gcode\""))
        .expect("the G-code type");
    for needed in [
        "<sub-class-of type=\"text/plain\"/>".to_owned(),
        format!("<icon name=\"{id}\"/>"),
        "<glob pattern=\"*.nc\"/>".to_owned(),
        "<glob pattern=\"*.ngc\"/>".to_owned(),
        "<glob pattern=\"*.tap\"/>".to_owned(),
    ] {
        assert!(gcode.contains(&needed), "the G-code type needs {needed}");
    }
    // The types it shares extensions with are defined here in full (KDE
    // would otherwise hide them), with signatures to tell them by content.
    for (other, glob) in [
        ("application/x-netcdf", "*.nc"),
        ("application/x-neo-geo-pocket-color-rom", "*.ngc"),
    ] {
        let def = mime
            .split("<mime-type ")
            .find(|t| t.starts_with(&format!("type=\"{other}\"")))
            .unwrap_or_else(|| panic!("{other} defined"));
        assert!(
            def.contains(&format!("<glob pattern=\"{glob}\"/>")) && def.contains("<magic"),
            "{other}"
        );
    }
    let desktop =
        std::fs::read_to_string(root.join(format!("{id}.desktop"))).expect("desktop file");
    assert!(
        desktop.lines().any(|l| l == "MimeType=text/x-cnc-gcode;"),
        "{desktop}"
    );
    assert!(
        desktop.lines().any(|l| l == "Exec=gcode-editsim %f"),
        "{desktop}"
    );
    let manifest = std::fs::read_to_string(root.join(format!("{id}.yml"))).expect("manifest");
    assert!(
        manifest.contains(&format!(
            "packaging/flatpak/{id}.xml /app/share/mime/packages/{id}.xml"
        )),
        "the manifest installs the MIME package"
    );
}
