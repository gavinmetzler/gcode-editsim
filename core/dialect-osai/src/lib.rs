//! Osai 10 Series milling dialect (stub).
//!
//! A deliberately small subset, written in M2 to check that the dialect
//! interfaces suit a controller other than Fanuc (ARCHITECTURE.md §12):
//! G00-G03, G17-G19, G70/G71, G90/G91, G79, G04, G94/G95, `(UAO,n)`
//! origins, F/S/T and the standard M codes. Codes are declared in
//! `data/osai.toml`.
//!
//! Every behaviour is taken from the Osai "10 Series CNC Programming
//! Manual" and cited in `docs/dialect-notes/osai.md`; what the manual
//! leaves open is marked *unverified* there. Anything outside the subset
//! produces a diagnostic. A code that would change later moves but is not
//! simulated (origins, presetting) gets one warning, and the moves after
//! it are still drawn, as programmed: moves are never hidden (owner
//! decision, 2026-10-31).

#![forbid(unsafe_code)]

mod block;
mod handler;
mod three_letter;

use std::collections::BTreeMap;

use dialect_api::{
    AxisSet, BlockContext, CodeKey, CodeTable, CodeTableError, Command, Dialect, ModalState,
};
use gcode_syntax::Block;
use ir::{ArcPlane, Diagnostics, LengthUnit, WorkOffsetId};
use thiserror::Error;

use crate::handler::Handler;

/// The code table shipped with this crate.
const TABLE: &str = include_str!("../data/osai.toml");

/// Why the dialect could not be created.
#[derive(Debug, Error)]
pub enum OsaiError {
    /// The code table is malformed.
    #[error(transparent)]
    Table(#[from] CodeTableError),
    /// The code table names a handler this crate does not implement.
    #[error("{code}: unknown handler {handler:?}")]
    UnknownHandler {
        /// The code.
        code: String,
        /// The handler name.
        handler: String,
    },
}

/// The Osai 10 Series dialect.
#[derive(Debug, Clone)]
pub struct Osai {
    table: CodeTable,
    handlers: BTreeMap<CodeKey, Handler>,
}

impl Osai {
    /// Creates the dialect.
    pub fn new() -> Result<Self, OsaiError> {
        let table = CodeTable::from_toml_str(TABLE)?;
        let mut handlers = BTreeMap::new();
        for def in table.iter() {
            let handler =
                Handler::from_name(&def.handler).ok_or_else(|| OsaiError::UnknownHandler {
                    code: def.code.to_string(),
                    handler: def.handler.clone(),
                })?;
            handlers.insert(def.code, handler);
        }
        Ok(Self { table, handlers })
    }

    pub(crate) fn handler(&self, code: CodeKey) -> Option<Handler> {
        self.handlers.get(&code).copied()
    }
}

/// The milling power-up codes (manual p. 1-11/1-12, "POWER UP" column).
/// The machine builder can change them in AMP, so the dialect warns once
/// when a program relies on units, distance mode or plane without setting
/// them.
const POWER_UP_CODES: &[&str] = &[
    "G00", "G17", "G27", "G99", "G40", "G71", "G80", "G90", "G94", "G97",
];

/// The plane of the axes at positions `a` and `b` in the axis list, if the
/// machine has them.
pub(crate) fn plane_by_position(axes: &AxisSet, a: u8, b: u8) -> Option<ArcPlane> {
    let n = axes.len();
    (usize::from(a) < n && usize::from(b) < n && a != b).then_some(ArcPlane {
        first: a,
        second: b,
    })
}

impl Dialect for Osai {
    fn id(&self) -> &str {
        self.table.id()
    }

    fn code_table(&self) -> &CodeTable {
        &self.table
    }

    fn initial_state(&self, axes: &AxisSet) -> ModalState {
        let mut state = ModalState::default();
        for text in POWER_UP_CODES {
            let def = CodeKey::parse(text).and_then(|k| self.table.get(k));
            if let Some((group, code)) = def.and_then(|d| Some((d.group?, d.code))) {
                state.set_active(group, code);
            }
        }
        state.units = LengthUnit::Metric;
        state.plane = plane_by_position(axes, 0, 1);
        // "At power-up ... all axes are referred to the home position"
        // (UAO, p. 2-95): machine coordinates.
        state.work_offset = WorkOffsetId(0);
        state
    }

    fn interpret_block(
        &self,
        block: &Block,
        ctx: &BlockContext<'_>,
        state: &mut ModalState,
        out: &mut Vec<Command>,
        diag: &mut Diagnostics,
    ) {
        block::interpret(self, block, ctx, state, out, diag);
    }
}
