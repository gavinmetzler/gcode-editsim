//! What the app remembers between sessions: the machine and the dialect
//! chosen last (M4.1), and the window layout (M4.2). Kept in a small JSON file, `settings.json`, in the
//! app's configuration folder next to the machine folder.
//!
//! Reading never fails: a missing file gives the defaults, and a damaged
//! one gives the defaults (or the settings that could still be read) with
//! a message saying what was wrong, so the app always starts.
//!
//! Settings this version does not know (written by a later version) are
//! kept as they were when the file is saved again.
//!
//! A user machine is remembered by its file name in the machine folder,
//! never by a full path, so an edited settings file cannot make the app
//! read a file outside that folder.

use std::fs;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::files::{self, MachineSource, SHIPPED_MACHINES};
use crate::BackendError;

/// The settings file's name, in the app's configuration folder.
pub const SETTINGS_FILE: &str = "settings.json";

/// The largest settings file the app reads, in bytes (64 KiB).
pub const MAX_SETTINGS_BYTES: u64 = 64 * 1024;

/// The version written to the file. Reading accepts any version and uses
/// the settings it understands.
pub const VERSION: u32 = 1;

/// A machine file, as remembered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum RememberedMachine {
    /// An example shipped with the app, by id.
    Shipped {
        /// The example's id ([`SHIPPED_MACHINES`]).
        id: String,
    },
    /// A file in the user's machine folder, by file name.
    User {
        /// The file name, without any folder.
        file: String,
    },
}

impl RememberedMachine {
    /// How to remember `source`. `None` for a user file that is not
    /// directly in `machine_dir` (the app lists no other files).
    pub fn from_source(source: &MachineSource, machine_dir: &Path) -> Option<Self> {
        match source {
            MachineSource::Shipped(id) => Some(Self::Shipped {
                id: (*id).to_owned(),
            }),
            MachineSource::User(path) => {
                if path.parent()? != machine_dir {
                    return None;
                }
                let file = path.file_name()?.to_str()?.to_owned();
                is_machine_file_name(&file).then_some(Self::User { file })
            }
        }
    }

    /// The machine file this names now: `None` for an example this version
    /// does not ship, or a file name that is not a plain `*.toml` name in
    /// `machine_dir`. Whether the file still exists is not checked here.
    pub fn to_source(&self, machine_dir: &Path) -> Option<MachineSource> {
        match self {
            Self::Shipped { id } => SHIPPED_MACHINES
                .iter()
                .find(|(i, _)| i == id)
                .map(|(i, _)| MachineSource::Shipped(i)),
            Self::User { file } => {
                is_machine_file_name(file).then(|| MachineSource::User(machine_dir.join(file)))
            }
        }
    }

    /// A name to show the user: the file name, or the example's id.
    pub fn label(&self) -> &str {
        match self {
            Self::Shipped { id } => id,
            Self::User { file } => file,
        }
    }
}

/// `true` for a plain file name ending in `.toml` (any case): one path
/// component, no folder, not `.` or `..`.
fn is_machine_file_name(name: &str) -> bool {
    let path = Path::new(name);
    let mut components = path.components();
    let single_normal =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    single_normal
        && !name.contains(['/', '\\'])
        && path
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("toml"))
}

/// The panes' sizes, the maximised pane, the colour key and the
/// diagnostics filters, as the UI last left them.
/// The UI gives the names and meanings; this only keeps them sensible.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PaneLayout {
    /// The editor's share of the window's width, between 0 and 1 (not
    /// including either); `None` for the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<f64>,
    /// The diagnostics panel's share of the right-hand column's height,
    /// between 0 and 1; `None` for the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<f64>,
    /// The maximised pane's name, or `None` for the normal layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximised: Option<String>,
    /// Whether the colour key in the view is folded (M4.2a); `None` for
    /// the default (open).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend_folded: Option<bool>,
    /// The diagnostics severities whose filter box is unticked (M4.2a),
    /// by the UI's names; `None` for the default (all shown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_diagnostics: Option<Vec<String>>,
}

impl PaneLayout {
    /// The layout if every share is a number between 0 and 1 (not
    /// including either), or a message naming the first that is not.
    pub fn checked(self) -> Result<Self, String> {
        for (name, share) in [("editor", self.editor), ("panel", self.panel)] {
            if let Some(v) = share {
                if !(v > 0.0 && v < 1.0) {
                    return Err(format!("the {name} pane size {v} is not between 0 and 1"));
                }
            }
        }
        Ok(self)
    }
}

/// The window's size, place and maximised state when it was last open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Inside width in logical pixels (the normal size, also when the
    /// window was maximised).
    pub width: f64,
    /// Inside height in logical pixels.
    pub height: f64,
    /// Left edge of the frame in physical desktop pixels; `None` where the
    /// desktop places windows itself (Wayland).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    /// Top edge of the frame in physical desktop pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    /// Whether it was maximised.
    #[serde(default)]
    pub maximized: bool,
}

/// The window's size when nothing usable is remembered, in logical pixels
/// (as in `tauri.conf.json`).
pub const DEFAULT_WINDOW: (f64, f64) = (1200.0, 800.0);

/// The smallest window size restored, in logical pixels (smaller only on a
/// screen smaller than this).
pub const MIN_WINDOW: (f64, f64) = (640.0, 400.0);

/// A screen's work area (without panels and docks), in physical desktop
/// pixels, and its scale factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorArea {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Physical pixels per logical pixel.
    pub scale: f64,
}

impl MonitorArea {
    fn contains(&self, x: i64, y: i64) -> bool {
        let (left, top) = (i64::from(self.x), i64::from(self.y));
        x >= left
            && x < left + i64::from(self.width)
            && y >= top
            && y < top + i64::from(self.height)
    }

    fn scale(&self) -> f64 {
        if self.scale.is_finite() && self.scale > 0.0 {
            self.scale
        } else {
            1.0
        }
    }

    /// The work area's size in logical pixels.
    fn logical_size(&self) -> (f64, f64) {
        let s = self.scale();
        (f64::from(self.width) / s, f64::from(self.height) / s)
    }
}

/// Where and how big to open the window.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// Inside width in logical pixels.
    pub width: f64,
    /// Inside height in logical pixels.
    pub height: f64,
    /// The frame's top-left corner in physical desktop pixels, or `None`
    /// to let the desktop place the window.
    pub position: Option<(i32, i32)>,
    /// Maximise it.
    pub maximized: bool,
}

/// Fits a remembered window onto the screens there are now (their work
/// areas). The size is kept if it fits the screen the window was on (or
/// the largest screen), and shrunk to it otherwise, never below
/// [`MIN_WINDOW`] unless the screen is smaller. The position is kept only
/// if the top of the window is on a screen, and moved so the window lies
/// on that screen; otherwise the desktop places it. Never off-screen.
pub fn place_window(saved: &WindowGeometry, monitors: &[MonitorArea]) -> Placement {
    let usable = |v: f64| v.is_finite() && v > 0.0;
    let (mut width, mut height) = if usable(saved.width) && usable(saved.height) {
        (
            saved.width.max(MIN_WINDOW.0),
            saved.height.max(MIN_WINDOW.1),
        )
    } else {
        DEFAULT_WINDOW
    };
    // The screen under the window's title bar, a little in from its corner.
    let on = match (saved.x, saved.y) {
        (Some(x), Some(y)) => monitors
            .iter()
            .find(|m| m.contains(i64::from(x) + 40, i64::from(y) + 15)),
        _ => None,
    };
    let largest = monitors.iter().max_by(|a, b| {
        let (aw, ah) = a.logical_size();
        let (bw, bh) = b.logical_size();
        (aw * ah).total_cmp(&(bw * bh))
    });
    if let Some(m) = on.or(largest) {
        let (mw, mh) = m.logical_size();
        width = width.min(mw);
        height = height.min(mh);
    }
    let position = match (on, saved.x, saved.y) {
        (Some(m), Some(x), Some(y)) => {
            let s = m.scale();
            let fit = |at: i32, start: i32, span: u32, size: f64| -> i32 {
                let start = i64::from(start);
                let last = start + i64::from(span) - (size * s).round() as i64;
                i64::from(at)
                    .min(last)
                    .max(start)
                    .clamp(i32::MIN.into(), i32::MAX.into()) as i32
            };
            Some((fit(x, m.x, m.width, width), fit(y, m.y, m.height, height)))
        }
        _ => None,
    };
    Placement {
        width,
        height,
        position,
        maximized: saved.maximized,
    }
}

/// What the app remembers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    /// The machine chosen last.
    pub machine: Option<RememberedMachine>,
    /// The id of the dialect chosen last.
    pub dialect: Option<String>,
    /// The panes as the UI last left them.
    pub layout: Option<PaneLayout>,
    /// The window when it was last open.
    pub window: Option<WindowGeometry>,
    /// Settings this version does not know, kept as they were.
    other: Map<String, Value>,
}

/// Settings read from a file, and what was wrong with it, if anything.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Loaded {
    /// The settings that could be read; the defaults for the rest.
    pub settings: Settings,
    /// Why some or all of the file could not be used.
    pub problem: Option<String>,
}

impl Settings {
    /// Settings with this machine and dialect, and nothing else.
    pub fn new(machine: Option<RememberedMachine>, dialect: Option<String>) -> Self {
        Self {
            machine,
            dialect,
            layout: None,
            window: None,
            other: Map::new(),
        }
    }

    /// Reads settings from the file's bytes. Never fails: what cannot be
    /// read is left at its default and described in [`Loaded::problem`].
    pub fn from_json(bytes: &[u8]) -> Loaded {
        let mut map = match serde_json::from_slice::<Value>(bytes) {
            Ok(Value::Object(map)) => map,
            Ok(_) => return damaged("not a JSON object".to_owned()),
            Err(e) => return damaged(format!("not valid JSON ({e})")),
        };
        let mut problems = Vec::new();
        map.remove("version");
        let machine = match map.remove("machine") {
            None | Some(Value::Null) => None,
            Some(v) => match serde_json::from_value::<RememberedMachine>(v) {
                Ok(m) => Some(m),
                Err(e) => {
                    problems.push(format!("the machine setting was not understood ({e})"));
                    None
                }
            },
        };
        let dialect = match map.remove("dialect") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s),
            Some(_) => {
                problems.push("the dialect setting is not text".to_owned());
                None
            }
        };
        let layout = match map.remove("layout") {
            None | Some(Value::Null) => None,
            Some(Value::Object(fields)) => Some(read_layout(&fields, &mut problems)),
            Some(_) => {
                problems.push("the layout setting is not understood".to_owned());
                None
            }
        };
        let window = match map.remove("window") {
            None | Some(Value::Null) => None,
            Some(v) => match serde_json::from_value::<WindowGeometry>(v) {
                Ok(w) if w.width > 0.0 && w.height > 0.0 => Some(w),
                Ok(_) => {
                    problems.push("the window size is not usable".to_owned());
                    None
                }
                Err(e) => {
                    problems.push(format!("the window setting was not understood ({e})"));
                    None
                }
            },
        };
        Loaded {
            settings: Settings {
                machine,
                dialect,
                layout,
                window,
                other: map,
            },
            problem: (!problems.is_empty()).then(|| problems.join("; ")),
        }
    }

    /// The settings as the file's JSON text.
    pub fn to_json(&self) -> String {
        let mut map = self.other.clone();
        map.insert("version".to_owned(), Value::from(VERSION));
        if let Some(m) = &self.machine {
            // Serialising a plain enum of strings cannot fail.
            if let Ok(v) = serde_json::to_value(m) {
                map.insert("machine".to_owned(), v);
            }
        }
        if let Some(d) = &self.dialect {
            map.insert("dialect".to_owned(), Value::String(d.clone()));
        }
        // Serialising plain numbers, strings and flags cannot fail.
        if let Some(l) = self
            .layout
            .as_ref()
            .and_then(|l| serde_json::to_value(l).ok())
        {
            map.insert("layout".to_owned(), l);
        }
        if let Some(w) = self
            .window
            .as_ref()
            .and_then(|w| serde_json::to_value(w).ok())
        {
            map.insert("window".to_owned(), w);
        }
        let mut text =
            serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_else(|_| "{}".to_owned());
        text.push('\n');
        text
    }
}

/// Reads the pane layout one setting at a time, so one bad value does not
/// lose the others.
fn read_layout(fields: &Map<String, Value>, problems: &mut Vec<String>) -> PaneLayout {
    let mut share = |name: &str| match fields.get(name) {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_f64() {
            Some(x) if x > 0.0 && x < 1.0 => Some(x),
            _ => {
                problems.push(format!(
                    "the {name} pane size is not a share between 0 and 1"
                ));
                None
            }
        },
    };
    let editor = share("editor");
    let panel = share("panel");
    let maximised = match fields.get("maximised") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            problems.push("the maximised pane is not a name".to_owned());
            None
        }
    };
    let legend_folded = match fields.get("legend_folded") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(_) => {
            problems.push("the colour key setting is not true or false".to_owned());
            None
        }
    };
    let hidden_diagnostics = match fields.get("hidden_diagnostics") {
        None | Some(Value::Null) => None,
        Some(v) => match serde_json::from_value::<Vec<String>>(v.clone()) {
            Ok(names) => Some(names),
            Err(_) => {
                problems.push("the diagnostics filters setting is not a list of names".to_owned());
                None
            }
        },
    };
    PaneLayout {
        editor,
        panel,
        maximised,
        legend_folded,
        hidden_diagnostics,
    }
}

fn damaged(why: String) -> Loaded {
    Loaded {
        settings: Settings::default(),
        problem: Some(why),
    }
}

/// Reads the settings file at `path`. A missing file gives the defaults
/// with no problem (the first start); an unreadable or damaged one gives
/// what could be read, with a problem naming the file.
pub fn load(path: &Path) -> Loaded {
    let bytes = match files::read_limited(path, MAX_SETTINGS_BYTES) {
        Ok(bytes) => bytes,
        Err(BackendError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Loaded::default();
        }
        Err(e) => return damaged(e.to_string()),
    };
    let mut loaded = Settings::from_json(&bytes);
    if let Some(p) = &mut loaded.problem {
        *p = format!("{}: {p}", path.display());
    }
    loaded
}

/// Writes the settings file at `path`, creating its folder if needed. The
/// file is replaced in one step, so a failed save leaves the old one.
pub fn save(path: &Path, settings: &Settings) -> Result<(), BackendError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| BackendError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }
    files::save_program(path, settings.to_json().as_bytes())
}

/// Reads the settings file (the defaults if it is missing or damaged),
/// changes it with `change`, and saves it, keeping every other setting.
pub fn update(path: &Path, change: impl FnOnce(&mut Settings)) -> Result<(), BackendError> {
    let mut settings = load(path).settings;
    change(&mut settings);
    save(path, &settings)
}
