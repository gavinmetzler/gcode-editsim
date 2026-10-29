//! A tiny made-up dialect, showing that the trait, code table, axis set
//! and helpers fit together. It is not a real controller.

use dialect_api::{
    literal, report_syntax_errors, AxisInfo, AxisSet, AxisWord, BlockContext, CodeKey, CodeTable,
    Command, Dialect, DistanceMode, ModalState,
};
use gcode_syntax::{parse_str, Block};
use ir::{DiagCode, Diagnostics, SourceRef};

const TABLE: &str = r#"
[dialect]
id = "example"

[[g]]
code = "G0"
group = 1
modal = true
handler = "rapid"

[[g]]
code = "G1"
group = 1
modal = true
handler = "linear"

[[g]]
code = "G90"
group = 3
modal = true
handler = "absolute"

[[g]]
code = "G91"
group = 3
modal = true
handler = "incremental"

[[m]]
code = "M0"
handler = "stop"

[[m]]
code = "M2"
handler = "program_end"
"#;

const MOTION_GROUP: u16 = 1;

struct Example {
    table: CodeTable,
}

impl Example {
    fn new() -> Self {
        Self {
            table: CodeTable::from_toml_str(TABLE).expect("example table is valid"),
        }
    }
}

impl Dialect for Example {
    fn id(&self) -> &str {
        self.table.id()
    }

    fn code_table(&self) -> &CodeTable {
        &self.table
    }

    fn initial_state(&self, _axes: &AxisSet) -> ModalState {
        let mut state = ModalState::default();
        if let Some(g0) = CodeKey::parse("G0") {
            state.set_active(MOTION_GROUP, g0);
        }
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
        if report_syntax_errors(block, ctx.src, diag) {
            return;
        }
        let mut axis_words = Vec::new();
        let mut after_motion = Vec::new();
        for word in &block.words {
            let Some(n) = literal(&word.value) else {
                diag.error(
                    DiagCode::InvalidValue,
                    "expressions are not supported",
                    ctx.src,
                );
                return;
            };
            if let Some(axis) = ctx.axes.index_of(word.letter) {
                axis_words.push(AxisWord {
                    axis,
                    value: n.value,
                    incremental: state.distance == DistanceMode::Incremental,
                });
                continue;
            }
            let def = CodeKey::from_value(word.letter, n.value).and_then(|k| self.table.get(k));
            match (word.letter, def) {
                ('F', _) => out.push(Command::SetFeedRate(n.value)),
                (_, Some(def)) => match def.handler.as_str() {
                    "rapid" | "linear" => state.set_active(MOTION_GROUP, def.code),
                    "absolute" => {
                        state.distance = DistanceMode::Absolute;
                        out.push(Command::SetDistanceMode(DistanceMode::Absolute));
                    }
                    "incremental" => {
                        state.distance = DistanceMode::Incremental;
                        out.push(Command::SetDistanceMode(DistanceMode::Incremental));
                    }
                    "stop" => after_motion.push(Command::Stop { optional: false }),
                    "program_end" => after_motion.push(Command::ProgramEnd { rewind: false }),
                    other => diag.error(DiagCode::InvalidValue, other.to_string(), ctx.src),
                },
                ('G', None) => {
                    diag.error(DiagCode::UnsupportedGCode, format!("G{}", n.value), ctx.src);
                    out.push(Command::Unsupported {
                        what: format!("G{}", n.value),
                    });
                }
                ('M', None) => {
                    diag.error(DiagCode::UnsupportedMCode, format!("M{}", n.value), ctx.src);
                    out.push(Command::Unsupported {
                        what: format!("M{}", n.value),
                    });
                }
                (letter, None) => {
                    diag.error(DiagCode::UnsupportedWord, letter.to_string(), ctx.src);
                }
            }
        }
        // Axis words use the incremental flag in force for the whole block.
        for w in &mut axis_words {
            w.incremental = state.distance == DistanceMode::Incremental;
        }
        if !axis_words.is_empty() {
            let rapid = state.active_code(MOTION_GROUP) == CodeKey::parse("G0");
            out.push(if rapid {
                Command::Rapid { target: axis_words }
            } else {
                Command::Linear { target: axis_words }
            });
        }
        out.extend(after_motion);
    }
}

fn run(src: &str) -> (Vec<Command>, Diagnostics) {
    let dialect = Example::new();
    let axes = AxisSet::new(vec![
        AxisInfo {
            letter: 'X',
            arc_offset_letter: None,
            rotary: false,
        },
        AxisInfo {
            letter: 'Y',
            arc_offset_letter: None,
            rotary: false,
        },
    ])
    .unwrap();
    let mut state = dialect.initial_state(&axes);
    let mut out = Vec::new();
    let mut diag = Diagnostics::new();
    for (index, block) in parse_str(src).blocks.iter().enumerate() {
        let ctx = BlockContext {
            axes: &axes,
            src: SourceRef {
                line: block.line,
                block: u32::try_from(index).unwrap(),
                ..SourceRef::default()
            },
        };
        dialect.interpret_block(block, &ctx, &mut state, &mut out, &mut diag);
    }
    (out, diag)
}

fn word(axis: usize, value: f64, incremental: bool) -> AxisWord {
    AxisWord {
        axis,
        value,
        incremental,
    }
}

#[test]
fn modal_motion_and_distance_mode_carry_across_blocks() {
    let (out, diag) = run("X1 Y2\nG1 F100 X3\nY4\nG91 X1\nM2\n");
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(
        out,
        vec![
            Command::Rapid {
                target: vec![word(0, 1.0, false), word(1, 2.0, false)]
            },
            Command::SetFeedRate(100.0),
            Command::Linear {
                target: vec![word(0, 3.0, false)]
            },
            Command::Linear {
                target: vec![word(1, 4.0, false)]
            },
            Command::SetDistanceMode(DistanceMode::Incremental),
            Command::Linear {
                target: vec![word(0, 1.0, true)]
            },
            Command::ProgramEnd { rewind: false },
        ]
    );
}

#[test]
fn unknown_codes_and_letters_are_reported_not_ignored() {
    let (out, diag) = run("G5 M99 Z1\n");
    let codes: Vec<DiagCode> = diag.iter().map(|d| d.code).collect();
    assert_eq!(
        codes,
        [
            DiagCode::UnsupportedGCode,
            DiagCode::UnsupportedMCode,
            DiagCode::UnsupportedWord
        ]
    );
    assert_eq!(out.len(), 2);
}

#[test]
fn syntax_errors_and_expressions_skip_the_block() {
    let (out, diag) = run("G1 X\nX#1\nM0\n");
    let lines: Vec<(u32, DiagCode)> = diag.iter().map(|d| (d.src.line, d.code)).collect();
    assert_eq!(
        lines,
        [(1, DiagCode::SyntaxError), (2, DiagCode::InvalidValue)]
    );
    assert_eq!(out, vec![Command::Stop { optional: false }]);
}

#[test]
fn trait_objects_work() {
    let dialect: Box<dyn Dialect> = Box::new(Example::new());
    assert_eq!(dialect.id(), "example");
    assert_eq!(dialect.code_table().len(), 6);
}
