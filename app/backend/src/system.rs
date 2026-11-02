//! What the diagnostics panel learns from the app side (M5.2): the build
//! target, the Flatpak runtime (which fixes the WebKitGTK version), the
//! graphics cards and drivers (WebKit hides the real GPU from WebGL), and
//! the environment variables that change how WebKitGTK and GTK draw.
//! Nothing personal: no paths, user names or other variables.
//!
//! [`system_info`] is pure: the shell passes in the environment and the
//! files' contents. [`graphics_cards`] reads a `/sys/class/drm` folder.

use std::ffi::OsString;
use std::path::Path;

use serde::Serialize;

/// The environment variables reported, in this order: the ones that
/// change how the window and the 3D view are drawn.
pub const DRAWING_ENV: &[&str] = &[
    "WEBKIT_DISABLE_DMABUF_RENDERER",
    "WEBKIT_DISABLE_COMPOSITING_MODE",
    "WEBKIT_FORCE_COMPOSITING_MODE",
    "GDK_BACKEND",
    "GSK_RENDERER",
    "XDG_SESSION_TYPE",
    "XDG_CURRENT_DESKTOP",
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "LIBGL_ALWAYS_SOFTWARE",
    "__GLX_VENDOR_LIBRARY_NAME",
    "__NV_PRIME_RENDER_OFFLOAD",
    "MESA_LOADER_DRIVER_OVERRIDE",
];

/// The facts, as sent to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// The operating system the app was built for (`linux`, `windows`, `macos`).
    pub os: String,
    /// The processor architecture (`x86_64`, `aarch64`).
    pub arch: String,
    /// The Flatpak runtime, such as `org.gnome.Platform/x86_64/47`, or
    /// `None` outside a Flatpak.
    pub flatpak_runtime: Option<String>,
    /// The graphics cards, as [`graphics_cards`] describes them.
    pub gpus: Vec<String>,
    /// The NVIDIA driver version, when one is loaded.
    pub nvidia_driver: Option<String>,
    /// The variables of [`DRAWING_ENV`] that are set, in that order.
    pub env: Vec<(String, String)>,
}

/// What the shell read from the system for [`system_info`].
#[derive(Debug, Clone, Default)]
pub struct SystemFacts<'a> {
    /// The contents of `/.flatpak-info`, inside a Flatpak.
    pub flatpak_info: Option<&'a str>,
    /// The graphics cards ([`graphics_cards`]).
    pub gpus: Vec<String>,
    /// The contents of `/proc/driver/nvidia/version`, when it exists.
    pub nvidia_proc: Option<&'a str>,
}

/// The facts from the process environment `vars` and what the shell read
/// from the system.
pub fn system_info(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
    facts: SystemFacts<'_>,
) -> SystemInfo {
    let set: Vec<(String, String)> = vars
        .into_iter()
        .filter_map(|(k, v)| {
            let k = k.into_string().ok()?;
            DRAWING_ENV
                .contains(&k.as_str())
                .then(|| (k, v.to_string_lossy().into_owned()))
        })
        .collect();
    let env = DRAWING_ENV
        .iter()
        .filter_map(|name| set.iter().find(|(k, _)| k == name).cloned())
        .collect();
    SystemInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        flatpak_runtime: facts.flatpak_info.and_then(flatpak_runtime),
        gpus: facts.gpus,
        nvidia_driver: facts.nvidia_proc.and_then(nvidia_driver),
        env,
    }
}

/// The runtime named in a Flatpak info file (`runtime=runtime/<name>` in
/// its `[Application]` group), without the leading `runtime/`.
pub fn flatpak_runtime(info: &str) -> Option<String> {
    let mut in_application = false;
    for line in info.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_application = line == "[Application]";
        } else if in_application {
            if let Some(value) = line.strip_prefix("runtime=") {
                let name = value.strip_prefix("runtime/").unwrap_or(value).trim();
                return (!name.is_empty()).then(|| name.to_string());
            }
        }
    }
    None
}

/// The graphics cards in a `/sys/class/drm` folder (`card0`, `card1`, but
/// not connectors such as `card1-HDMI-A-1`), sorted by name, each as
/// `card1: driver nvidia, PCI 10de:2684`. Empty when the folder cannot be
/// read (not Linux, or a sandbox without it).
pub fn graphics_cards(drm: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(drm) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| {
            n.strip_prefix("card")
                .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let dev = drm.join(&name).join("device");
            let id = |file: &str| {
                std::fs::read_to_string(dev.join(file))
                    .ok()
                    .map(|t| t.trim().trim_start_matches("0x").to_string())
                    .unwrap_or_else(|| "?".to_string())
            };
            let driver = std::fs::read_link(dev.join("driver"))
                .ok()
                .and_then(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_string))
                .unwrap_or_else(|| "unknown".to_string());
            format!(
                "{name}: driver {driver}, PCI {}:{}",
                id("vendor"),
                id("device")
            )
        })
        .collect()
}

/// The driver version in `/proc/driver/nvidia/version`: the first word
/// of its `NVRM version:` line made of digits and dots, with a dot.
pub fn nvidia_driver(text: &str) -> Option<String> {
    let line = text.lines().find(|l| l.starts_with("NVRM version:"))?;
    line.split_whitespace()
        .find(|w| w.contains('.') && w.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
        .map(str::to_string)
}
