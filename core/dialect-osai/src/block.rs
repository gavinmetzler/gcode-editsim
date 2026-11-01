//! Interpretation of one block.
//!
//! Order of effects within a block (see `docs/dialect-notes/osai.md`):
//! 1. three-letter commands such as `(UAO,1)`,
//! 2. G-code mode changes (units first, then the rest in written order),
//! 3. F, S and T values,
//! 4. M codes that act before motion (M3, M4, M6, M7, M8, M9, M13, M14,
//!    M19),
//! 5. the motion itself,
//! 6. M codes that act after motion (M0, M1, M2, M5, M30).
//!
//! Motion is never shown in the wrong place. Whenever the control would
//! move but this stub cannot say where to (a line it cannot read, an
//! unsupported mode, an arc form it does not handle), every axis is
//! reported as having an unknown position, so the interpreter does not
//! draw later moves from a stale starting point.

use dialect_api::{
    length_to_internal, literal, report_syntax_errors, ArcCenter, ArcDir, AxisWord, BlockContext,
    CodeKey, Command, CoolantState, DistanceMode, FeedMode, ModalState, SpindleState,
};
use gcode_syntax::{Block, CommentKind, Number, Span};
use ir::{ArcPlane, DiagCode, Diagnostic, Diagnostics, LengthUnit, SourceRef, WorkOffsetId};

use crate::handler::Handler;
use crate::three_letter::{self, ThreeLetter, NO_PATH_EFFECT};
use crate::{plane_by_position, Osai};

const MOTION_GROUP: u16 = 1;
const SPINDLE_MODE_GROUP: u16 = 13;

/// Marks (in `ModalState::reported_once`) that the program chose units or
/// a plane itself, so the power-up default need not be pointed out.
const SET_UNITS: &str = "osai.set.units";
const SET_PLANE: &str = "osai.set.plane";

/// The canned cycle group (power-up G80).
const CANNED_CYCLE_GROUP: u16 = 7;

/// A G or M code found in the block.
struct Code {
    key: CodeKey,
    handler: Handler,
    span: Span,
}

/// Any other word: a letter with a plain numeric value.
struct Value {
    letter: char,
    number: Number,
    span: Span,
}

/// How a G code affects the block's ordinary motion.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Motion {
    /// No effect: axis words move the machine as usual.
    Normal,
    /// The code uses the axis words itself, so there is no ordinary move.
    Consumed,
    /// A canned cycle: the block's position in the working plane is a hole,
    /// drawn as a rapid.
    CycleHoles,
}

/// The source reference narrowed to a span within the line.
fn at(src: SourceRef, span: Span) -> SourceRef {
    let col = |c: usize| u16::try_from(c).unwrap_or(u16::MAX);
    SourceRef {
        col_start: col(span.start),
        col_end: col(span.end),
        ..src
    }
}

fn description(osai: &Osai, key: CodeKey) -> String {
    match osai.table.get(key).and_then(|d| d.description.as_deref()) {
        Some(desc) => format!("{key} ({desc})"),
        None => key.to_string(),
    }
}

/// A command marking every axis's position as unknown.
fn all_unknown(ctx: &BlockContext<'_>, reason: String) -> Command {
    Command::PositionUnknown {
        axes: (0..ctx.axes.len()).collect(),
        reason,
    }
}

pub(crate) fn interpret(
    osai: &Osai,
    block: &Block,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let src = ctx.src;
    if report_syntax_errors(block, src, diag) {
        unreadable(block, ctx, state, out, diag);
        return;
    }
    for a in &block.assignments {
        diag.warning(
            DiagCode::NotYetSimulated,
            "variable assignment is not simulated",
            at(src, a.span),
        );
    }

    // 1. Three-letter commands.
    for comment in block
        .comments
        .iter()
        .filter(|c| c.kind == CommentKind::Paren)
    {
        let src = at(src, comment.span);
        match three_letter::parse(&comment.text) {
            Some(command) => three_letter_command(&command, src, state, out, diag),
            None => diag.push(
                Diagnostic::warning(
                    DiagCode::UnsupportedWord,
                    format!(
                        "({}) is not a three-letter command and was ignored; on a 10 Series \
                         control parentheses hold commands, and comments start with ';'",
                        comment.text
                    ),
                    src,
                )
                .dialect_specific(),
            ),
        }
    }

    // Validate the words before producing anything from them.
    let mut gcodes = Vec::new();
    let mut mcodes = Vec::new();
    let mut unknown = Vec::new();
    let mut values: Vec<Value> = Vec::new();
    for w in &block.words {
        let Some(number) = literal(&w.value) else {
            diag.error(
                DiagCode::NotYetSimulated,
                format!(
                    "{}: variables and expressions are not supported; block skipped",
                    w.letter
                ),
                at(src, w.span),
            );
            out.push(all_unknown(
                ctx,
                format!("line {}, which uses a variable", src.line),
            ));
            return;
        };
        match w.letter {
            'G' | 'M' => match CodeKey::from_value(w.letter, number.value)
                .and_then(|k| Some((k, osai.handler(k)?)))
            {
                Some((key, handler)) => {
                    let code = Code {
                        key,
                        handler,
                        span: w.span,
                    };
                    if w.letter == 'G' {
                        gcodes.push(code);
                    } else {
                        mcodes.push(code);
                    }
                }
                None => unknown.push((w.letter, number, w.span)),
            },
            letter => {
                if values.iter().any(|v| v.letter == letter) {
                    diag.error(
                        DiagCode::InvalidValue,
                        format!("address {letter} appears more than once; block skipped"),
                        at(src, w.span),
                    );
                    return;
                }
                values.push(Value {
                    letter,
                    number,
                    span: w.span,
                });
            }
        }
    }
    if !values_valid(&values, src, diag) || !codes_compatible(osai, &gcodes, &values, ctx, diag) {
        return;
    }

    // Unknown codes. What an unknown G code does is unknown: this block is
    // not moved (its words may be data), and later moves are drawn as
    // programmed, never hidden.
    let mut motion_effect = Motion::Normal;
    for (letter, number, span) in &unknown {
        let what = format!("{letter}{}", number.value);
        if *letter == 'G' {
            diag.error(
                DiagCode::UnsupportedGCode,
                format!(
                    "{what} is not supported and is ignored: this block is not moved, and \
                     later moves are drawn as programmed, so they may differ from the machine"
                ),
                at(src, *span),
            );
            motion_effect = Motion::Consumed;
        } else {
            diag.error(
                DiagCode::UnsupportedMCode,
                format!("{what} is not supported"),
                at(src, *span),
            );
        }
        out.push(Command::Unsupported { what });
    }

    // 2. G-code mode changes: units first, so this block's values use them.
    for code in &gcodes {
        let unit = match code.handler {
            Handler::UnitsInch => LengthUnit::Inch,
            Handler::UnitsMetric => LengthUnit::Metric,
            _ => continue,
        };
        state.units = unit;
        state.first_report(SET_UNITS);
        set_active(osai, state, code.key);
        out.push(Command::SetUnits(unit));
    }
    let mut g79 = None;
    let mut dwell = None;
    for code in &gcodes {
        match code.handler {
            Handler::MachineCoords => g79 = Some(code),
            Handler::Dwell => dwell = Some(code),
            _ => {}
        }
        let effect = apply_gcode(osai, code, ctx, state, out, diag);
        motion_effect = match (motion_effect, effect) {
            (Motion::Consumed, _) | (_, Motion::Consumed) => Motion::Consumed,
            (Motion::CycleHoles, _) | (_, Motion::CycleHoles) => Motion::CycleHoles,
            _ => Motion::Normal,
        };
    }

    // 3. F, S, T.
    apply_values(&values, src, state, out, diag);

    // 4. M codes before motion.
    for code in mcodes.iter().filter(|c| c.handler.before_motion()) {
        apply_mcode(code, ctx, state, out, diag);
    }

    // 5. Motion, then a G04 dwell at its end.
    motion(osai, &values, motion_effect, g79, ctx, state, out, diag);
    if let Some(code) = dwell {
        if state.first_report("osai.dwell_time") {
            diag.push(
                Diagnostic::warning(
                    DiagCode::NotYetSimulated,
                    "G04 dwells for the time set by DWT, which is not read; the dwell is shown \
                     without its time. Further cases are not reported.",
                    at(src, code.span),
                )
                .dialect_specific(),
            );
        }
        out.push(Command::Dwell { seconds: 0.0 });
    }

    // 6. M codes after motion.
    for code in mcodes.iter().filter(|c| !c.handler.before_motion()) {
        apply_mcode(code, ctx, state, out, diag);
    }

    report_unused_words(osai, &gcodes, &values, ctx, state, diag);
}

/// A line the parser could not read. The 10 Series accepts syntax this
/// parser does not (labels, axis names without a value, E-parameter
/// assignments, `>>`), so the control may have moved here.
fn unreadable(
    block: &Block,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let g16 = block.words.iter().find(|w| {
        w.letter == 'G' && literal(&w.value).is_some_and(|n| n.value == 16.0 && !n.decimal_point)
    });
    if let Some(w) = g16 {
        // "G16 X A" names the plane's axes, which this parser cannot read.
        state.plane = None;
        diag.push(
            Diagnostic::error(
                DiagCode::UnsupportedGCode,
                "G16 (plane by axis names) is not supported; arcs are not shown until G17, \
                 G18 or G19",
                at(ctx.src, w.span),
            )
            .dialect_specific(),
        );
    }
    out.push(all_unknown(
        ctx,
        format!("line {}, which could not be read", ctx.src.line),
    ));
}

fn three_letter_command(
    command: &ThreeLetter,
    src: SourceRef,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let name = command.name.as_str();
    if NO_PATH_EFFECT.contains(&name) {
        return;
    }
    if name == "UAO" {
        // (UAO,n) selects origin n for every axis; 0 is the home position.
        let n = match command.args.as_slice() {
            [n] => n.parse::<u16>().ok().filter(|&n| n <= 10),
            _ => None,
        };
        // (UAO,n,axes...) selects origin n for the named axes only. Not
        // simulated yet (M3.5); soft-ignored at the owner's request: the
        // current origin stays in use and moves keep being drawn.
        if let [n, axes @ ..] = command.args.as_slice() {
            if !axes.is_empty() && n.parse::<u16>().is_ok_and(|n| n <= 10) {
                diag.push(
                    Diagnostic::warning(
                        DiagCode::NotYetSimulated,
                        format!(
                            "({name},{}) is not simulated yet: the current origin stays in \
                             use for {}, so moves on those axes may be drawn offset",
                            command.args.join(","),
                            axes.join(", ")
                        ),
                        src,
                    )
                    .dialect_specific(),
                );
                return;
            }
        }
        if let Some(n) = n {
            let id = WorkOffsetId(n);
            state.work_offset = id;
            out.push(Command::SetWorkOffset(id));
            return;
        }
        // Not an origin this stub can read: ignored, and moves are never
        // hidden (owner decision, 2026-10-31).
        diag.push(
            Diagnostic::warning(
                DiagCode::NotYetSimulated,
                format!(
                    "(UAO,{}) is not simulated and is ignored (only (UAO,n) with n from 0 \
                         to 10 is): the current origin stays in use, so later moves may be \
                         drawn offset",
                    command.args.join(",")
                ),
                src,
            )
            .dialect_specific(),
        );
        out.push(Command::Unsupported {
            what: format!("({name})"),
        });
        return;
    }
    // Rotation, scaling, mirroring, other origins, subroutines, jumps and
    // everything else not supported may change where later blocks move. It
    // is ignored with a warning: moves are never hidden (owner decision,
    // 2026-10-31).
    let written = if command.args.is_empty() {
        format!("({name})")
    } else {
        format!("({name},{})", command.args.join(","))
    };
    diag.push(
        Diagnostic::warning(
            DiagCode::NotYetSimulated,
            format!(
                "{written} is not simulated and is ignored: later moves are drawn without its \
                 effect, so they may differ from the machine"
            ),
            src,
        )
        .dialect_specific(),
    );
    out.push(Command::Unsupported {
        what: format!("({name})"),
    });
}

/// Checks F, S and T values. Returns `false` (after reporting) if the
/// block must be skipped.
fn values_valid(values: &[Value], src: SourceRef, diag: &mut Diagnostics) -> bool {
    for v in values {
        let n = v.number.value;
        let problem = match v.letter {
            'F' if n <= 0.0 => "feed rate must be greater than zero",
            'S' if n < 0.0 => "spindle speed cannot be negative",
            // T: tool code before the point, offset number after it.
            'T' if n < 0.0 || n.trunc() > f64::from(u32::MAX) => {
                "tool codes above 4294967295 or below 0 are not supported"
            }
            _ => continue,
        };
        diag.error(
            DiagCode::InvalidValue,
            format!("{problem}; block skipped"),
            at(src, v.span),
        );
        return false;
    }
    true
}

/// Codes the manual's compatibility table (p. 1-10) forbids together.
/// Checked here: two codes of one group, and G17-G19 or G99 with anything
/// else (they must be alone in their block, p. 2-47). The rest of the
/// table is not checked.
fn codes_compatible(
    osai: &Osai,
    gcodes: &[Code],
    values: &[Value],
    ctx: &BlockContext<'_>,
    diag: &mut Diagnostics,
) -> bool {
    let group_of = |k: CodeKey| osai.table.get(k).and_then(|d| d.group);
    for (i, a) in gcodes.iter().enumerate() {
        for b in gcodes.iter().skip(i + 1) {
            if group_of(a.key).is_some() && group_of(a.key) == group_of(b.key) {
                diag.push(
                    Diagnostic::error(
                        DiagCode::ModalGroupConflict,
                        format!(
                            "{} and {} cannot be in the same block; block skipped",
                            a.key, b.key
                        ),
                        at(ctx.src, b.span),
                    )
                    .dialect_specific(),
                );
                return false;
            }
        }
        let alone = matches!(a.handler, Handler::Plane(..))
            || (a.handler == Handler::NoEffect && group_of(a.key) == Some(4));
        if alone && (gcodes.len() > 1 || !values.is_empty()) {
            diag.push(
                Diagnostic::error(
                    DiagCode::ModalGroupConflict,
                    format!("{} must be alone in its block; block skipped", a.key),
                    at(ctx.src, a.span),
                )
                .dialect_specific(),
            );
            return false;
        }
    }
    true
}

fn set_active(osai: &Osai, state: &mut ModalState, key: CodeKey) {
    if let Some(def) = osai.table.get(key) {
        if let (true, Some(group)) = (def.modal, def.group) {
            state.set_active(group, key);
        }
    }
}

/// Warns, once per program, that a power-up default the machine builder
/// can change is being relied on.
fn note_power_up(
    state: &mut ModalState,
    set_key: &str,
    what: &str,
    src: SourceRef,
    diag: &mut Diagnostics,
) {
    if state.reported_once.contains(set_key) || !state.first_report(&format!("{set_key}.noted")) {
        return;
    }
    diag.push(
        Diagnostic::warning(
            DiagCode::ControllerDependent,
            format!(
                "assumed {what}: the program has not selected it, so the 10 Series power-up \
                 default is used. The machine builder can change this default (AMP); add it \
                 near the start of the program to make it explicit."
            ),
            src,
        )
        .dialect_specific(),
    );
}

/// Applies one G code (other than units) and says how it affects the
/// block's motion.
fn apply_gcode(
    osai: &Osai,
    code: &Code,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) -> Motion {
    let src = at(ctx.src, code.span);
    match code.handler {
        Handler::Rapid | Handler::Linear | Handler::ArcCw | Handler::ArcCcw => {
            set_active(osai, state, code.key);
        }
        Handler::Plane(a, b) => {
            state.first_report(SET_PLANE);
            match plane_by_position(ctx.axes, a, b) {
                Some(plane) => {
                    note_axis_order(ctx, state, src, diag);
                    state.plane = Some(plane);
                    set_active(osai, state, code.key);
                    out.push(Command::SetPlane(plane));
                }
                None => {
                    state.plane = None;
                    diag.error(
                        DiagCode::InvalidValue,
                        format!(
                            "{} needs axes {} and {} in the machine's axis order, which this \
                             machine does not have; arcs are not shown until a plane is set",
                            code.key,
                            a + 1,
                            b + 1
                        ),
                        src,
                    );
                }
            }
        }
        Handler::Absolute | Handler::Incremental => {
            let mode = if code.handler == Handler::Absolute {
                DistanceMode::Absolute
            } else {
                DistanceMode::Incremental
            };
            state.distance = mode;
            set_active(osai, state, code.key);
            out.push(Command::SetDistanceMode(mode));
        }
        Handler::FeedPerMinute => {
            state.feed_mode = FeedMode::PerMinute;
            set_active(osai, state, code.key);
            out.push(Command::SetFeedMode(FeedMode::PerMinute));
        }
        Handler::FeedPerRevolution => {
            state.feed_mode = FeedMode::PerRevolution;
            set_active(osai, state, code.key);
            out.push(Command::SetFeedMode(FeedMode::PerRevolution));
        }
        Handler::NoEffect => set_active(osai, state, code.key),
        Handler::NotSimulated => {
            set_active(osai, state, code.key);
            diag.warning(
                DiagCode::NotYetSimulated,
                format!(
                    "{} is not simulated yet; the path is shown without it",
                    description(osai, code.key)
                ),
                src,
            );
        }
        Handler::Ignored => {
            set_active(osai, state, code.key);
            diag.push(
                Diagnostic::warning(
                    DiagCode::UnsupportedGCode,
                    format!(
                        "{} is not simulated and is ignored: its values are not a move, and \
                         later moves are drawn without its effect, so they may differ from the \
                         machine",
                        description(osai, code.key)
                    ),
                    src,
                )
                .dialect_specific(),
            );
            out.push(Command::Unsupported {
                what: code.key.to_string(),
            });
            // Axis words with these codes are data (a preset).
            return Motion::Consumed;
        }
        Handler::CannedCycle => {
            set_active(osai, state, code.key);
            if state.first_report(&format!("osai.cycle.{}", code.key)) {
                diag.push(
                    Diagnostic::warning(
                        DiagCode::UnsupportedGCode,
                        format!(
                            "{} is not simulated: its hole positions are drawn as rapids in the \
                             working plane; depths, R levels and retracts are not drawn. Not \
                             reported again.",
                            description(osai, code.key)
                        ),
                        src,
                    )
                    .dialect_specific(),
                );
            }
            out.push(Command::Unsupported {
                what: code.key.to_string(),
            });
            return Motion::CycleHoles;
        }
        // Handled with the block's motion; units were applied earlier; M
        // handlers never appear as G codes.
        _ => {}
    }
    Motion::Normal
}

/// G17-G19 use the first three axes in the machine's order. The manual
/// says they are valid when the axes are configured as X, Y, Z (p. 2-46);
/// warn once if they are not.
fn note_axis_order(
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    src: SourceRef,
    diag: &mut Diagnostics,
) {
    let letters: String = ctx.axes.axes().iter().take(3).map(|a| a.letter).collect();
    if letters != "XYZ" && state.first_report("osai.axis_order") {
        diag.push(
            Diagnostic::warning(
                DiagCode::ControllerDependent,
                format!(
                    "G17, G18 and G19 use the first three axes in the machine's order ({letters} \
                     here); the 10 Series manual describes them for axes configured as X, Y, Z"
                ),
                src,
            )
            .dialect_specific(),
        );
    }
}

/// Applies F, S and T (already checked by [`values_valid`]).
fn apply_values(
    values: &[Value],
    src: SourceRef,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    for v in values {
        let n = v.number.value;
        match v.letter {
            'F' => {
                note_power_up(state, SET_UNITS, "G71 (mm)", at(src, v.span), diag);
                let feed = length_to_internal(n, state.units);
                state.feed_rate = Some(feed);
                out.push(Command::SetFeedRate(feed));
            }
            'S' => {
                let surface_speed = state
                    .active_code(SPINDLE_MODE_GROUP)
                    .is_some_and(|k| k.value() == 96.0);
                if surface_speed {
                    diag.warning(
                        DiagCode::NotYetSimulated,
                        "S is a surface speed under G96, which is not simulated; the spindle \
                         speed is left unchanged",
                        at(src, v.span),
                    );
                } else {
                    state.spindle_rpm = n;
                    out.push(Command::SetSpindleSpeed { spindle: 0, rpm: n });
                }
            }
            'T' => {
                // The tool code is the whole part; the digits after the
                // point are the offset number, not simulated. In range
                // (checked earlier), so the cast is exact.
                let tool = n.trunc() as u32;
                state.selected_tool = Some(tool);
                out.push(Command::SelectTool(tool));
            }
            _ => {}
        }
    }
}

fn apply_mcode(
    code: &Code,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let src = at(ctx.src, code.span);
    let spindle = |out: &mut Vec<Command>, state: &mut ModalState, s: SpindleState| {
        state.spindle_state = s;
        out.push(Command::SetSpindleState {
            spindle: 0,
            state: s,
        });
    };
    match code.handler {
        Handler::SpindleCw => spindle(out, state, SpindleState::Cw),
        Handler::SpindleCcw => spindle(out, state, SpindleState::Ccw),
        Handler::SpindleStop | Handler::SpindleOrient => spindle(out, state, SpindleState::Off),
        Handler::SpindleCwCoolant => {
            spindle(out, state, SpindleState::Cw);
            out.push(Command::Coolant(CoolantState::Flood));
        }
        Handler::SpindleCcwCoolant => {
            spindle(out, state, SpindleState::Ccw);
            out.push(Command::Coolant(CoolantState::Flood));
        }
        // The IR has flood and mist; the auxiliary coolant is shown as mist.
        Handler::CoolantAuxiliary => out.push(Command::Coolant(CoolantState::Mist)),
        Handler::CoolantMain => out.push(Command::Coolant(CoolantState::Flood)),
        Handler::CoolantOff => out.push(Command::Coolant(CoolantState::Off)),
        Handler::ToolChange => match state.selected_tool {
            Some(tool) => {
                state.current_tool = Some(tool);
                out.push(Command::ToolChange { tool });
            }
            None => diag.error(
                DiagCode::MissingWord,
                format!("{} with no tool selected (no T word yet)", code.key),
                src,
            ),
        },
        Handler::Stop => out.push(Command::Stop { optional: false }),
        Handler::OptionalStop => out.push(Command::Stop { optional: true }),
        Handler::ProgramEnd => out.push(Command::ProgramEnd { rewind: false }),
        Handler::ProgramEndRewind => out.push(Command::ProgramEnd { rewind: true }),
        _ => {}
    }
}

/// Converts a length or angle word to internal units. The 10 Series reads
/// numbers as written, with or without a decimal point (`X10` is 10 mm).
fn convert(
    v: &Value,
    rotary: bool,
    state: &mut ModalState,
    diag: &mut Diagnostics,
    src: SourceRef,
) -> f64 {
    if rotary {
        return v.number.value;
    }
    note_power_up(state, SET_UNITS, "G71 (mm)", at(src, v.span), diag);
    length_to_internal(v.number.value, state.units)
}

fn is_rotary(ctx: &BlockContext<'_>, axis: usize) -> bool {
    ctx.axes.axes().get(axis).is_some_and(|a| a.rotary)
}

/// The block's axis words as a target.
fn axis_target(
    values: &[Value],
    incremental: bool,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    diag: &mut Diagnostics,
) -> Vec<AxisWord> {
    let mut target = Vec::new();
    for v in values {
        let Some(axis) = ctx.axes.index_of(v.letter) else {
            continue;
        };
        target.push(AxisWord {
            axis,
            value: convert(v, is_rotary(ctx, axis), state, diag, ctx.src),
            incremental,
        });
    }
    target
}

/// The canned cycle in effect, if any (until G80).
fn cycle_active(osai: &Osai, state: &ModalState) -> bool {
    state
        .active_code(CANNED_CYCLE_GROUP)
        .is_some_and(|k| osai.handler(k) == Some(Handler::CannedCycle))
}

/// A canned cycle's hole: a rapid to the position the block gives in the
/// working plane. The depth and the cycle's own words are not drawn.
fn cycle_hole(
    values: &[Value],
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let Some(plane) = state.plane else {
        return; // No working plane: nothing to draw.
    };
    note_power_up(
        state,
        SET_PLANE,
        "G17 (plane of axes 1 and 2)",
        ctx.src,
        diag,
    );
    let incremental = state.distance == DistanceMode::Incremental;
    let in_plane = |a: usize| a == usize::from(plane.first) || a == usize::from(plane.second);
    let target: Vec<AxisWord> = axis_target(values, incremental, ctx, state, diag)
        .into_iter()
        .filter(|w| in_plane(w.axis))
        .collect();
    if !target.is_empty() {
        out.push(Command::Rapid { target });
    }
}

#[allow(clippy::too_many_arguments)]
fn motion(
    osai: &Osai,
    values: &[Value],
    effect: Motion,
    g79: Option<&Code>,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let src = ctx.src;
    let find = |letter: char| values.iter().find(|v| v.letter == letter);
    let (mode_key, mode) = match state
        .active_code(MOTION_GROUP)
        .and_then(|k| Some((k, osai.handler(k)?)))
    {
        Some(m) => m,
        None => return, // Never: G00 is active from power-up.
    };
    let is_arc = mode.is_arc();
    let has_axes = values.iter().any(|v| ctx.axes.index_of(v.letter).is_some());
    let (i, j, r, k) = (find('I'), find('J'), find('R'), find('K'));

    // Outside an arc, R is most likely a lower-case r (a corner radius
    // between moves), which the parser reads as R.
    if let (Some(r), false) = (r, is_arc) {
        diag.push(
            Diagnostic::warning(
                DiagCode::UnsupportedWord,
                "R outside an arc is not simulated (a lower-case r rounds the corner to the next \
                 move): the corner is drawn sharp",
                at(src, r.span),
            )
            .dialect_specific(),
        );
    }

    // An arc with no axes is a full circle (p. 2-4: "If axes are not
    // programmed in the block, the move is a complete circle").
    let wants_motion = has_axes || (is_arc && (i.is_some() || j.is_some() || r.is_some()));
    if !wants_motion {
        return;
    }
    match effect {
        Motion::Consumed => return,
        Motion::CycleHoles => return cycle_hole(values, ctx, state, out, diag),
        Motion::Normal if cycle_active(osai, state) => {
            return cycle_hole(values, ctx, state, out, diag);
        }
        Motion::Normal => {}
    }

    if let Some(code) = g79 {
        // G79: coordinates refer to the home position (machine zero here),
        // for this block only, always absolute.
        let target = axis_target(values, false, ctx, state, diag);
        if mode == Handler::Rapid {
            out.push(Command::MachineRapid { target });
        } else {
            diag.error(
                DiagCode::NotYetSimulated,
                format!("G79 with {mode_key} is not supported, only with G00"),
                at(src, code.span),
            );
            out.push(all_unknown(ctx, format!("G79 on line {}", src.line)));
        }
        return;
    }

    let incremental = state.distance == DistanceMode::Incremental;
    let target = axis_target(values, incremental, ctx, state, diag);
    let feed_ok = |state: &ModalState, diag: &mut Diagnostics| {
        // "If no feedrate has been programmed the control will generate an
        // error" (G01, G02/G03 syntax).
        let ok = state.feed_rate.is_some();
        if !ok {
            diag.error(
                DiagCode::MissingFeedRate,
                format!("{mode_key} with no feed rate (F) programmed; block not moved"),
                src,
            );
        }
        ok
    };

    match mode {
        Handler::Rapid => out.push(Command::Rapid { target }),
        Handler::Linear => {
            if feed_ok(state, diag) {
                out.push(Command::Linear { target });
            }
        }
        Handler::ArcCw | Handler::ArcCcw => {
            let dir = if mode == Handler::ArcCw {
                ArcDir::Cw
            } else {
                ArcDir::Ccw
            };
            let Some(plane) = state.plane else {
                diag.error(
                    DiagCode::InvalidArc,
                    "no arc plane is selected (after G16, or this machine lacks the axes); arc \
                     not shown",
                    src,
                );
                out.push(all_unknown(
                    ctx,
                    format!("the arc on line {} in an unknown plane", src.line),
                ));
                return;
            };
            note_power_up(state, SET_PLANE, "G17 (plane of axes 1 and 2)", src, diag);
            if let Some(k) = k {
                diag.push(
                    Diagnostic::error(
                        DiagCode::NotYetSimulated,
                        "helix pitch K is not supported; arc not shown",
                        at(src, k.span),
                    )
                    .dialect_specific(),
                );
                out.push(all_unknown(ctx, format!("the helix on line {}", src.line)));
                return;
            }
            let center = match arc_center(plane, i, j, r, target.is_empty(), ctx, state, diag) {
                Ok(center) => center,
                Err(Rejected::ControlError) => return,
                Err(Rejected::Unknown) => {
                    out.push(all_unknown(ctx, format!("the arc on line {}", src.line)));
                    return;
                }
            };
            if feed_ok(state, diag) {
                out.push(Command::Arc {
                    target,
                    center,
                    dir,
                    plane,
                });
            }
        }
        _ => {}
    }
}

/// Why an arc was not produced.
enum Rejected {
    /// The control itself refuses the block, so the machine does not move.
    ControlError,
    /// The control would move, but this stub does not know where.
    Unknown,
}

/// The arc's centre. I and J are the centre on the plane's first and
/// second axes whatever the plane (p. 2-4): a position in G90, a distance
/// from the start in G91. R is the radius, negative for more than 180°.
#[allow(clippy::too_many_arguments)]
fn arc_center(
    plane: ArcPlane,
    i: Option<&Value>,
    j: Option<&Value>,
    r: Option<&Value>,
    no_end_point: bool,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    diag: &mut Diagnostics,
) -> Result<ArcCenter, Rejected> {
    let src = ctx.src;
    if let Some(r) = r {
        if i.is_some() || j.is_some() {
            diag.push(
                Diagnostic::error(
                    DiagCode::InvalidArc,
                    "R and I/J in one arc: the manual gives R as an alternative to I and J; arc \
                     not shown",
                    at(src, r.span),
                )
                .dialect_specific(),
            );
            return Err(Rejected::Unknown);
        }
        if no_end_point {
            diag.push(
                Diagnostic::error(
                    DiagCode::InvalidArc,
                    "R cannot program a full circle; block not moved",
                    at(src, r.span),
                )
                .dialect_specific(),
            );
            return Err(Rejected::ControlError);
        }
        return Ok(ArcCenter::Radius(convert(r, false, state, diag, src)));
    }
    let incremental = state.distance == DistanceMode::Incremental;
    let mut words = Vec::new();
    for (axis, v) in [(plane.first, i), (plane.second, j)] {
        let axis = usize::from(axis);
        match v {
            Some(v) => words.push(AxisWord {
                axis,
                value: convert(v, is_rotary(ctx, axis), state, diag, src),
                incremental,
            }),
            None if incremental => {} // No distance: the start's coordinate.
            None => {
                diag.push(
                    Diagnostic::error(
                        DiagCode::MissingWord,
                        "an arc centre in absolute mode (G90) needs both I and J; arc not shown",
                        src,
                    )
                    .dialect_specific(),
                );
                return Err(Rejected::Unknown);
            }
        }
    }
    if words.is_empty() {
        diag.error(
            DiagCode::InvalidArc,
            "arc needs I and J, or R; block not moved",
            src,
        );
        return Err(Rejected::ControlError);
    }
    Ok(ArcCenter::Position(words))
}

/// Warns about words that nothing in the block used.
fn report_unused_words(
    osai: &Osai,
    gcodes: &[Code],
    values: &[Value],
    ctx: &BlockContext<'_>,
    state: &ModalState,
    diag: &mut Diagnostics,
) {
    let arc_mode = state
        .active_code(MOTION_GROUP)
        .and_then(|k| osai.handler(k))
        .is_some_and(Handler::is_arc);
    let declared = |letter: char| {
        gcodes.iter().any(|c| {
            osai.table.get(c.key).is_some_and(|d| {
                d.optional_words.contains(&letter) || d.required_words.contains(&letter)
            })
        })
    };
    for v in values {
        let used = matches!(v.letter, 'F' | 'S' | 'T' | 'R')
            || ctx.axes.index_of(v.letter).is_some()
            || (arc_mode && matches!(v.letter, 'I' | 'J' | 'K'))
            || declared(v.letter);
        if !used {
            diag.warning(
                DiagCode::UnsupportedWord,
                format!(
                    "{}{} has no effect in this block and was ignored",
                    v.letter, v.number.value
                ),
                at(ctx.src, v.span),
            );
        }
    }
}
