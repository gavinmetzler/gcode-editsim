//! Everything the desktop app does, without Tauri.
//!
//! The Tauri shell (`app/shell`) only forwards its commands here, so all of
//! the app's behaviour can be tested without a window:
//!
//! - [`load_machine`]: reads a machine file and picks the machine type its
//!   `family` names.
//! - [`dialects`] / [`load_dialect`]: the dialects the user can choose.
//! - [`interpret_to_buffer`]: runs a program and returns the render buffer
//!   bytes (ARCHITECTURE.md §8), never JSON.
//! - [`files`]: reading and saving programs and listing machine files.
//!   Unlike `core/`, this crate may use the file system.

#![forbid(unsafe_code)]

pub mod files;

use dialect_api::Dialect;
use dialect_fanuc::Fanuc;
use dialect_osai::Osai;
use interpreter::{interpret, Options};
use ir::{encode_render_buffer, EncodeError, DEFAULT_TOLERANCE};
use machine_api::{Machine, MachineDef, MachineDefError};
use machine_lathe::Lathe;
use machine_mill::Mill;
use thiserror::Error;

/// Why the app could not do what was asked.
#[derive(Debug, Error)]
pub enum BackendError {
    /// The machine file is not valid (bad TOML, an unknown setting, or
    /// rules of its machine type broken).
    #[error("machine file: {0}")]
    Machine(String),
    /// The machine file names a machine type this app does not have.
    #[error("machine file: unknown family {0:?} (known: mill, lathe)")]
    UnknownFamily(String),
    /// A dialect id this app does not have.
    #[error("unknown dialect {0:?}")]
    UnknownDialect(String),
    /// The arc tolerance is not a number greater than zero.
    #[error("arc tolerance must be a number greater than zero")]
    InvalidTolerance,
    /// The program is too large for the render buffer.
    #[error(transparent)]
    Encode(#[from] EncodeError),
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: String,
        /// What went wrong.
        source: std::io::Error,
    },
    /// A file is larger than the app reads.
    #[error("{path}: larger than {limit} bytes")]
    FileTooLarge {
        /// The file.
        path: String,
        /// The limit in bytes.
        limit: u64,
    },
}

impl From<MachineDefError> for BackendError {
    fn from(e: MachineDefError) -> Self {
        BackendError::Machine(e.to_string())
    }
}

/// A machine, of whichever type its file named.
pub type AnyMachine = Box<dyn Machine + Send + Sync>;

/// Reads a machine file and builds the machine type its `family` names.
pub fn load_machine(toml: &str) -> Result<AnyMachine, BackendError> {
    let def = MachineDef::from_toml_str(toml)?;
    let machine: AnyMachine = match def.family.as_str() {
        machine_mill::FAMILY => Box::new(Mill::new(def).map_err(machine_error)?),
        machine_lathe::FAMILY => Box::new(Lathe::new(def).map_err(machine_error)?),
        other => return Err(BackendError::UnknownFamily(other.to_owned())),
    };
    Ok(machine)
}

fn machine_error(e: impl std::fmt::Display) -> BackendError {
    BackendError::Machine(e.to_string())
}

/// A dialect the user can choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialectInfo {
    /// Stable id, used to ask for it.
    pub id: &'static str,
    /// Name to show.
    pub name: &'static str,
    /// `true` for an early, partial implementation.
    pub stub: bool,
}

/// The dialects, in the order to offer them.
pub fn dialects() -> &'static [DialectInfo] {
    &[
        DialectInfo {
            id: "fanuc",
            name: "Fanuc (milling)",
            stub: false,
        },
        DialectInfo {
            id: "osai",
            name: "Osai 10 Series (milling, early stub)",
            stub: true,
        },
    ]
}

/// The dialect with the given id.
pub fn load_dialect(id: &str) -> Result<Box<dyn Dialect + Send + Sync>, BackendError> {
    // The shipped code tables are tested, so loading them only fails if
    // this crate was built wrongly; report it rather than panic.
    let broken = |e: &dyn std::fmt::Display| BackendError::UnknownDialect(format!("{id}: {e}"));
    match id {
        "fanuc" => Ok(Box::new(Fanuc::new().map_err(|e| broken(&e))?)),
        "osai" => Ok(Box::new(Osai::new().map_err(|e| broken(&e))?)),
        _ => Err(BackendError::UnknownDialect(id.to_owned())),
    }
}

/// Settings for one run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunOptions {
    /// The operator's block-delete switch.
    pub block_delete: bool,
    /// Arc tolerance for drawing, in mm (chord error).
    pub tolerance: f64,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            block_delete: false,
            tolerance: DEFAULT_TOLERANCE,
        }
    }
}

/// Interprets `program` with the dialect `dialect_id` on the machine
/// described by `machine_toml`, and returns the render buffer.
///
/// Problems in the program are diagnostics inside the buffer, not errors;
/// an error means the run could not happen at all (bad machine file,
/// unknown dialect, bad tolerance, or a program too large to encode).
pub fn interpret_to_buffer(
    program: &[u8],
    file_name: &str,
    machine_toml: &str,
    dialect_id: &str,
    options: RunOptions,
) -> Result<Vec<u8>, BackendError> {
    if !(options.tolerance > 0.0 && options.tolerance.is_finite()) {
        return Err(BackendError::InvalidTolerance);
    }
    let machine = load_machine(machine_toml)?;
    let dialect = load_dialect(dialect_id)?;
    let result = interpret(
        program,
        file_name,
        dialect.as_ref(),
        machine.as_ref(),
        Options {
            block_delete: options.block_delete,
            ..Options::default()
        },
    );
    let buffer = encode_render_buffer(&result, options.tolerance, |p| {
        machine.forward_kinematics(p).position
    })?;
    Ok(buffer)
}
