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
mod numbers;
mod three_letter;

use std::collections::BTreeMap;

use dialect_api::{
    AxisSet, BlockContext, CodeKey, CodeTable, CodeTableError, Command, Dialect, DistanceMode,
    ModalState, PowerOn, StartMotion, ValueKind,
};
use gcode_syntax::{Block, Number, NumberStyle};
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

    /// The machine file's power-on state (M5.6b), as the codes that select
    /// it: G70/G71, the plane code whose axis positions match the stated
    /// letters on this machine (G17 1-2, G18 3-1, G19 2-3; another pair is
    /// left to the program), G00/G01 and G90/G91. Units and plane then
    /// count as set, so they are not reported as power-up defaults.
    fn apply_power_on(&self, state: &mut ModalState, axes: &AxisSet, power_on: &PowerOn) {
        let activate = |state: &mut ModalState, text: &str| {
            let def = CodeKey::parse(text).and_then(|k| self.table.get(k));
            if let Some((group, code)) = def.and_then(|d| Some((d.group?, d.code))) {
                state.set_active(group, code);
            }
        };
        if let Some(units) = power_on.units {
            activate(
                state,
                if units == LengthUnit::Inch {
                    "G70"
                } else {
                    "G71"
                },
            );
            state.units = units;
            state.first_report(block::SET_UNITS);
        }
        if let Some((a, b)) = power_on.plane {
            let positions = (axes.index_of(a), axes.index_of(b));
            let code = self
                .handlers
                .iter()
                .find_map(|(key, handler)| match (handler, positions) {
                    (Handler::Plane(x, y), (Some(i), Some(j)))
                        if usize::from(*x) == i && usize::from(*y) == j =>
                    {
                        Some((*key, *x, *y))
                    }
                    _ => None,
                });
            if let Some((key, x, y)) = code {
                if let Some(plane) = plane_by_position(axes, x, y) {
                    if let Some(group) = self.table.get(key).and_then(|d| d.group) {
                        state.set_active(group, key);
                    }
                    state.plane = Some(plane);
                    state.first_report(block::SET_PLANE);
                }
            }
        }
        if let Some(motion) = power_on.motion {
            activate(
                state,
                if motion == StartMotion::Linear {
                    "G01"
                } else {
                    "G00"
                },
            );
        }
        if let Some(distance) = power_on.distance {
            activate(
                state,
                if distance == DistanceMode::Incremental {
                    "G91"
                } else {
                    "G90"
                },
            );
            state.distance = distance;
        }
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

    /// Numbers as written, with or without a point (manual p. 2-4).
    fn read_value(&self, number: &Number, kind: ValueKind, units: LengthUnit) -> Option<f64> {
        Some(numbers::read(number, kind, units))
    }

    /// As written: a number without a point gains one only when the new
    /// value is not whole; rounded to 0.001 mm or degree, 0.0001 inch.
    fn format_value(
        &self,
        value: f64,
        kind: ValueKind,
        units: LengthUnit,
        style: &NumberStyle,
    ) -> Option<String> {
        numbers::format(value, kind, units, style)
    }

    /// `R`, negative for more than 180° (p. 2-6).
    fn arc_radius_letter(&self) -> Option<char> {
        Some('R')
    }

    /// I and J are the plane's first and second axis, whatever the plane
    /// (p. 2-4).
    fn arc_centre_letters(
        &self,
        _plane: ArcPlane,
        _machine: [Option<char>; 2],
    ) -> [Option<char>; 2] {
        [Some('I'), Some('J')]
    }

    /// Presetting (G92, G98, G99), canned cycles (and every block while
    /// one is active), G79, and three-letter commands that shift, turn,
    /// scale or mirror coordinates, including `(UAO,n,axes)`.
    fn transform_refusal(&self, block: &Block, state: &ModalState) -> Option<String> {
        block::transform_refusal(self, block, state)
    }
}
