//! Dialect trait, command enum, modal state and code-table loader.
//!
//! A *dialect* turns parsed G-code blocks ([`gcode_syntax::Block`]) into
//! machine-neutral [`Command`]s, following one controller family's rules.
//! This crate holds what every dialect shares; it contains no
//! controller-specific behaviour.
//!
//! - [`Dialect`]: the trait each dialect implements.
//! - [`Command`]: what a dialect produces.
//! - [`ModalState`]: what it remembers between blocks.
//! - [`CodeTable`]: its supported codes, loaded from TOML data.
//! - [`AxisSet`]: the machine's axes, so dialects never assume axis letters.
//!
//! All values in commands are in internal units (mm, degrees, mm/min, rpm,
//! seconds); dialects convert at this boundary.

#![forbid(unsafe_code)]

mod axes;
mod code;
mod command;
mod dialect;
mod helpers;
mod state;
mod table;

pub use axes::{AxisInfo, AxisSet, AxisSetError};
pub use code::CodeKey;
pub use command::{ArcCenter, ArcDir, AxisOffset, AxisWord, Command};
pub use dialect::{BlockContext, Dialect, ValueKind};
pub use helpers::{length_to_internal, literal, report_syntax_errors, MM_PER_INCH};
pub use state::{CoolantState, DistanceMode, FeedMode, ModalState, SpindleState};
pub use table::{CodeDef, CodeTable, CodeTableError};
