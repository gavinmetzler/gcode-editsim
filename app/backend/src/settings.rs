//! What the app remembers between sessions (M4.1): the machine and the
//! dialect chosen last. Kept in a small JSON file, `settings.json`, in the
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

/// What the app remembers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    /// The machine chosen last.
    pub machine: Option<RememberedMachine>,
    /// The id of the dialect chosen last.
    pub dialect: Option<String>,
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
        Loaded {
            settings: Settings {
                machine,
                dialect,
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
        let mut text =
            serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_else(|_| "{}".to_owned());
        text.push('\n');
        text
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
