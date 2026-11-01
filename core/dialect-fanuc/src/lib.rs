//! Fanuc milling dialect (0i-M style).
//!
//! Turns parsed G-code blocks into [`dialect_api::Command`]s following
//! Fanuc rules: G0–G3, G17–G19, G20/G21, G28/G30, G53, G54–G59, G90/G91,
//! G94/G95, F/S/T, the common M codes (M0, M1, M2, M3, M4, M5, M6, M8,
//! M9, M30) and subprogram calls (M98/M99). Canned cycles draw their hole
//! positions only; tool length compensation is noted, never simulated; a
//! few codes that would shift later moves (G92, G52, G10, ...) are ignored
//! with a warning. Codes are declared in `data/fanuc.toml`; this crate
//! holds the behaviour behind them.
//!
//! Anything else produces a diagnostic. Behaviour that
//! differs between Fanuc controls or machine builders follows the common
//! documented default and is listed in `docs/dialect-notes/fanuc.md`.

#![forbid(unsafe_code)]

mod block;
mod handler;

use std::collections::BTreeMap;

use dialect_api::{
    AxisSet, BlockContext, CodeKey, CodeTable, CodeTableError, Command, Dialect, ModalState,
};
use gcode_syntax::Block;
use ir::{Diagnostics, LengthUnit, WorkOffsetId};
use thiserror::Error;

use crate::handler::Handler;

/// The code table shipped with this crate.
const TABLE: &str = include_str!("../data/fanuc.toml");

/// How numbers written without a decimal point are read (Fanuc parameter
/// DPI, No. 3401 bit 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecimalInput {
    /// Fanuc's factory default (DPI = 0): `X10` means 10 least input
    /// increments, i.e. 0.010 mm or 0.0010 inch.
    #[default]
    LeastIncrement,
    /// Calculator-type input (DPI = 1): `X10` means 10 mm or 10 inch.
    Calculator,
}

/// Settings that differ between Fanuc machines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FanucOptions {
    /// How numbers without a decimal point are read.
    pub decimal_input: DecimalInput,
}

/// Why the dialect could not be created.
#[derive(Debug, Error)]
pub enum FanucError {
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

/// The Fanuc dialect.
#[derive(Debug, Clone)]
pub struct Fanuc {
    table: CodeTable,
    handlers: BTreeMap<CodeKey, Handler>,
    options: FanucOptions,
}

impl Fanuc {
    /// Creates the dialect with default options.
    pub fn new() -> Result<Self, FanucError> {
        Self::with_options(FanucOptions::default())
    }

    /// Creates the dialect with the given options.
    pub fn with_options(options: FanucOptions) -> Result<Self, FanucError> {
        let table = CodeTable::from_toml_str(TABLE)?;
        let mut handlers = BTreeMap::new();
        for def in table.iter() {
            let handler =
                Handler::from_name(&def.handler).ok_or_else(|| FanucError::UnknownHandler {
                    code: def.code.to_string(),
                    handler: def.handler.clone(),
                })?;
            handlers.insert(def.code, handler);
        }
        Ok(Self {
            table,
            handlers,
            options,
        })
    }

    /// The options in use.
    pub fn options(&self) -> FanucOptions {
        self.options
    }

    pub(crate) fn handler(&self, code: CodeKey) -> Option<Handler> {
        self.handlers.get(&code).copied()
    }
}

/// Codes active when the control is switched on.
///
/// Motion mode (group 1), plane (2), distance mode (3) and units (6) are
/// left unset: they depend on parameters or on the state at power-off, so
/// the dialect warns when a program relies on them without setting them,
/// and then assumes G00, G17, G90 and G21.
const POWER_ON_CODES: &[&str] = &["G94", "G40", "G49", "G80", "G54", "G98"];

impl Dialect for Fanuc {
    fn id(&self) -> &str {
        self.table.id()
    }

    fn code_table(&self) -> &CodeTable {
        &self.table
    }

    fn initial_state(&self, axes: &AxisSet) -> ModalState {
        let mut state = ModalState::default();
        for text in POWER_ON_CODES {
            let def = CodeKey::parse(text).and_then(|k| self.table.get(k));
            if let Some((group, code)) = def.and_then(|d| Some((d.group?, d.code))) {
                state.set_active(group, code);
            }
        }
        state.units = LengthUnit::Metric;
        state.plane = axes.plane('X', 'Y');
        state.work_offset = WorkOffsetId(1);
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

    /// `O1000` or `:1000` (the parser reads `:` as `O`) as the first word
    /// of a block starts program 1000.
    fn program_number(&self, block: &Block) -> Option<u32> {
        let word = block.words.first().filter(|w| w.letter == 'O')?;
        let n = dialect_api::literal(&word.value)?.value;
        (n >= 0.0 && n.fract() == 0.0 && n <= f64::from(u32::MAX)).then_some(n as u32)
    }
}
