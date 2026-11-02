//! What the diagnostics panel learns from the app side (M5.2).

use std::ffi::OsString;

use app_backend::system::{
    flatpak_runtime, graphics_cards, nvidia_driver, system_info, SystemFacts, DRAWING_ENV,
};

fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v)))
        .collect()
}

#[test]
fn keeps_only_the_variables_that_change_drawing_in_a_fixed_order() {
    let info = system_info(
        vars(&[
            ("HOME", "/home/someone"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("USER", "someone"),
            ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
        ]),
        SystemFacts::default(),
    );
    assert_eq!(
        info.env,
        vec![
            (
                "WEBKIT_DISABLE_DMABUF_RENDERER".to_string(),
                "1".to_string()
            ),
            ("XDG_SESSION_TYPE".to_string(), "wayland".to_string()),
        ]
    );
}

#[test]
fn never_reports_personal_variables() {
    for name in DRAWING_ENV {
        for personal in ["HOME", "USER", "LOGNAME", "PATH", "PWD"] {
            assert_ne!(*name, personal);
        }
    }
}

#[test]
fn reports_the_build_target() {
    let info = system_info(Vec::new(), SystemFacts::default());
    assert_eq!(info.os, std::env::consts::OS);
    assert_eq!(info.arch, std::env::consts::ARCH);
    assert_eq!(info.flatpak_runtime, None);
}

#[test]
fn reads_the_flatpak_runtime_from_its_info_file() {
    let info = "[Application]\nname=com.ncsoftbyai.gcodeeditsim\nruntime=runtime/org.gnome.Platform/x86_64/47\n\n[Instance]\nruntime-commit=abc\n";
    assert_eq!(
        flatpak_runtime(info),
        Some("org.gnome.Platform/x86_64/47".to_string())
    );
    assert_eq!(
        system_info(
            Vec::new(),
            SystemFacts {
                flatpak_info: Some(info),
                ..Default::default()
            }
        )
        .flatpak_runtime,
        Some("org.gnome.Platform/x86_64/47".to_string())
    );
}

#[test]
fn a_missing_or_odd_info_file_gives_no_runtime() {
    assert_eq!(flatpak_runtime(""), None);
    assert_eq!(flatpak_runtime("[Application]\nname=x\n"), None);
    assert_eq!(flatpak_runtime("[Instance]\nruntime=elsewhere\n"), None);
    assert_eq!(flatpak_runtime("[Application]\nruntime=\n"), None);
}

#[test]
fn values_that_are_not_unicode_are_shown_lossily_not_dropped() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let info = system_info(
            vec![(
                OsString::from("GDK_BACKEND"),
                OsString::from_vec(vec![b'x', 0xff]),
            )],
            SystemFacts::default(),
        );
        assert_eq!(info.env.len(), 1);
        assert!(info.env[0].1.starts_with('x'));
    }
}

/// A fake `/sys/class/drm` with the given cards: (name, vendor, device, driver).
fn drm(name: &str, cards: &[(&str, &str, &str, Option<&str>)]) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("editsim-drm-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (card, vendor, device, driver) in cards {
        let dev = root.join(card).join("device");
        std::fs::create_dir_all(&dev).expect("dirs");
        std::fs::write(dev.join("vendor"), format!("{vendor}\n")).expect("vendor");
        std::fs::write(dev.join("device"), format!("{device}\n")).expect("device");
        if let Some(d) = driver {
            // The real `driver` is a link to /sys/bus/pci/drivers/<name>.
            let target = root.join("drivers").join(d);
            std::fs::create_dir_all(&target).expect("driver dir");
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, dev.join("driver")).expect("link");
        }
    }
    // A connector, which is not a card.
    std::fs::create_dir_all(root.join("card1-HDMI-A-1")).expect("connector");
    root
}

#[test]
fn lists_the_graphics_cards_with_their_drivers_and_pci_ids() {
    let root = drm(
        "two",
        &[
            ("card1", "0x10de", "0x2684", Some("nvidia")),
            ("card0", "0x8086", "0xa780", None),
        ],
    );
    let cards = graphics_cards(&root);
    #[cfg(unix)]
    assert_eq!(
        cards,
        vec![
            "card0: driver unknown, PCI 8086:a780".to_string(),
            "card1: driver nvidia, PCI 10de:2684".to_string(),
        ]
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn no_drm_folder_means_no_cards() {
    assert!(graphics_cards(std::path::Path::new("/nonexistent/drm")).is_empty());
}

#[test]
fn reads_the_nvidia_driver_version() {
    let text = "NVRM version: NVIDIA UNIX x86_64 Kernel Module  560.35.03  Fri Aug 16 21:39:15 UTC 2024\nGCC version:  gcc version 13.2.0\n";
    assert_eq!(nvidia_driver(text), Some("560.35.03".to_string()));
    assert_eq!(
        nvidia_driver(
            "NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  565.57.01  Release Build\n"
        ),
        Some("565.57.01".to_string())
    );
    assert_eq!(nvidia_driver(""), None);
    assert_eq!(nvidia_driver("something else\n"), None);
}

#[test]
fn passes_the_cards_and_driver_through() {
    let info = system_info(
        Vec::new(),
        SystemFacts {
            flatpak_info: None,
            gpus: vec!["card0: driver amdgpu, PCI 1002:744c".to_string()],
            nvidia_proc: Some("NVRM version: NVIDIA UNIX x86_64 Kernel Module  550.1  x\n"),
            ..Default::default()
        },
    );
    assert_eq!(
        info.gpus,
        vec!["card0: driver amdgpu, PCI 1002:744c".to_string()]
    );
    assert_eq!(info.nvidia_driver, Some("550.1".to_string()));
}

#[test]
fn lists_the_graphics_device_links_by_pci_address() {
    let root = std::env::temp_dir().join(format!("editsim-bypath-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("dir");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("../renderD128", root.join("pci-0000:01:00.0-render"))
            .expect("link");
        std::os::unix::fs::symlink("../card0", root.join("pci-0000:01:00.0-card")).expect("link");
        assert_eq!(
            app_backend::system::drm_devices(&root),
            vec![
                "pci-0000:01:00.0-card -> card0".to_string(),
                "pci-0000:01:00.0-render -> renderD128".to_string(),
            ]
        );
    }
    let _ = std::fs::remove_dir_all(&root);
    assert!(app_backend::system::drm_devices(std::path::Path::new("/nonexistent")).is_empty());
}

#[test]
fn finds_gbm_backends_in_the_flatpak_gl_extensions() {
    let root = std::env::temp_dir().join(format!("editsim-gl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let gbm = root.join("nvidia-580-178-04").join("lib").join("gbm");
    std::fs::create_dir_all(&gbm).expect("dirs");
    std::fs::write(gbm.join("nvidia-drm_gbm.so"), b"").expect("file");
    std::fs::create_dir_all(root.join("default").join("lib")).expect("default");
    assert_eq!(
        app_backend::system::gbm_backends(&root),
        vec!["nvidia-580-178-04/lib/gbm/nvidia-drm_gbm.so".to_string()]
    );
    let _ = std::fs::remove_dir_all(&root);
    assert!(app_backend::system::gbm_backends(std::path::Path::new("/nonexistent")).is_empty());
}

#[test]
fn reports_where_gbm_looks_for_backends() {
    assert!(DRAWING_ENV.contains(&"GBM_BACKENDS_PATH"));
}
