//! Helpers shared by the conformance tests.

#![allow(dead_code)]

use dialect_api::{AxisInfo, AxisSet, AxisWord, BlockContext, Command, Dialect};
use dialect_fanuc::{Fanuc, FanucOptions};
use ir::{DiagCode, Diagnostic, Diagnostics, Severity, SourceRef};

/// A 3-axis machine: X/Y/Z with arc offsets I/J/K.
pub fn xyz() -> AxisSet {
    axes(&[
        ('X', Some('I'), false),
        ('Y', Some('J'), false),
        ('Z', Some('K'), false),
    ])
}

/// An axis set from `(letter, arc offset letter, rotary)` triples.
pub fn axes(spec: &[(char, Option<char>, bool)]) -> AxisSet {
    AxisSet::new(
        spec.iter()
            .map(|&(letter, arc_offset_letter, rotary)| AxisInfo {
                letter,
                arc_offset_letter,
                rotary,
            })
            .collect(),
    )
    .expect("valid axes")
}

/// Commands and diagnostics produced by a program.
///
/// Most tests leave units, plane, distance and motion mode unset to stay
/// short, so the "assumed ..." warnings for those are kept apart in
/// `assumed`; `tests/assumed_defaults.rs` checks them.
pub struct Run {
    pub commands: Vec<Command>,
    pub diags: Vec<Diagnostic>,
    pub assumed: Vec<Diagnostic>,
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

    /// Motion commands and reference returns.
    pub fn moves_and_returns(&self) -> Vec<&Command> {
        self.commands
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Command::Rapid { .. }
                        | Command::Linear { .. }
                        | Command::Arc { .. }
                        | Command::ReturnToReference { .. }
                        | Command::MachineRapid { .. }
                )
            })
            .collect()
    }

    /// Only the motion commands.
    pub fn moves(&self) -> Vec<&Command> {
        self.commands
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Command::Rapid { .. } | Command::Linear { .. } | Command::Arc { .. }
                )
            })
            .collect()
    }
}

pub fn run(src: &str) -> Run {
    run_with(FanucOptions::default(), &xyz(), src)
}

pub fn run_with(options: FanucOptions, axes: &AxisSet, src: &str) -> Run {
    let dialect = Fanuc::with_options(options).expect("table loads");
    let mut state = dialect.initial_state(axes);
    let mut commands = Vec::new();
    let mut diag = Diagnostics::new();
    for (index, block) in gcode_syntax::parse_str(src).blocks.iter().enumerate() {
        let ctx = BlockContext {
            axes,
            src: SourceRef {
                line: block.line,
                block: u32::try_from(index).unwrap_or(u32::MAX),
                ..SourceRef::default()
            },
        };
        dialect.interpret_block(block, &ctx, &mut state, &mut commands, &mut diag);
    }
    let (assumed, diags) = diag.into_vec().into_iter().partition(|d| {
        d.code == DiagCode::ControllerDependent && d.message.starts_with("assumed ")
    });
    Run {
        commands,
        diags,
        assumed,
    }
}

pub fn w(axis: usize, value: f64) -> AxisWord {
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

/// Asserts two floats are equal to within rounding error.
pub fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

/// The target of a motion command.
pub fn target(cmd: &Command) -> &[AxisWord] {
    match cmd {
        Command::Rapid { target } | Command::Linear { target } | Command::Arc { target, .. } => {
            target
        }
        other => panic!("not a move: {other:?}"),
    }
}
