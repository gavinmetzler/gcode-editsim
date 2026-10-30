//! Helpers shared by the conformance tests.

#![allow(dead_code)]

use dialect_api::{AxisInfo, AxisSet, AxisWord, BlockContext, Command, Dialect};
use dialect_osai::Osai;
use ir::{DiagCode, Diagnostic, Diagnostics, Severity, SourceRef};

/// X, Y, Z with no arc-offset letters: the 10 Series names the centre I
/// and J whatever the plane, so the machine's arc-offset letters are not
/// used.
pub fn xyz() -> AxisSet {
    axes(&[('X', false), ('Y', false), ('Z', false)])
}

/// An axis set from `(letter, rotary)` pairs.
pub fn axes(spec: &[(char, bool)]) -> AxisSet {
    AxisSet::new(
        spec.iter()
            .map(|&(letter, rotary)| AxisInfo {
                letter,
                arc_offset_letter: None,
                rotary,
            })
            .collect(),
    )
    .expect("valid axes")
}

/// Commands and diagnostics produced by a program.
pub struct Run {
    pub commands: Vec<Command>,
    pub diags: Vec<Diagnostic>,
}

impl Run {
    pub fn codes(&self) -> Vec<DiagCode> {
        self.diags.iter().map(|d| d.code).collect()
    }

    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect()
    }

    pub fn has(&self, code: DiagCode, text: &str) -> bool {
        self.diags
            .iter()
            .any(|d| d.code == code && d.message.contains(text))
    }

    /// Only the motion commands.
    pub fn moves(&self) -> Vec<&Command> {
        self.commands
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Command::Rapid { .. }
                        | Command::Linear { .. }
                        | Command::Arc { .. }
                        | Command::MachineRapid { .. }
                )
            })
            .collect()
    }

    /// `true` if some command marks positions unknown.
    pub fn position_lost(&self) -> bool {
        self.commands
            .iter()
            .any(|c| matches!(c, Command::PositionUnknown { .. }))
    }
}

/// Runs `src` on an X/Y/Z machine.
pub fn run(src: &str) -> Run {
    run_on(src, &xyz())
}

/// Runs `src` with the given axes.
pub fn run_on(src: &str, axes: &AxisSet) -> Run {
    let osai = Osai::new().expect("dialect loads");
    let parsed = gcode_syntax::parse(src.as_bytes());
    let mut state = osai.initial_state(axes);
    let mut commands = Vec::new();
    let mut diag = Diagnostics::new();
    for (i, block) in parsed.blocks.iter().enumerate() {
        let ctx = BlockContext {
            axes,
            src: SourceRef {
                line: block.line,
                block: u32::try_from(i).unwrap_or(u32::MAX),
                ..SourceRef::default()
            },
        };
        osai.interpret_block(block, &ctx, &mut state, &mut commands, &mut diag);
    }
    Run {
        commands,
        diags: diag.into_vec(),
    }
}

pub fn abs(axis: usize, value: f64) -> AxisWord {
    AxisWord {
        axis,
        value,
        incremental: false,
    }
}

pub fn inc(axis: usize, value: f64) -> AxisWord {
    AxisWord {
        axis,
        value,
        incremental: true,
    }
}
