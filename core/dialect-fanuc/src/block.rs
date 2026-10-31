//! Interpretation of one block.
//!
//! Order of effects within a block (see `docs/dialect-notes/fanuc.md`):
//! 1. G-code mode changes (units first, then the rest in written order),
//! 2. F, S and T values,
//! 3. M codes that act before motion (M3, M4, M6, M8),
//! 4. the motion itself,
//! 5. M codes that act after motion (M0, M1, M2, M5, M9, M30).
//!
//! Motion is never shown in the wrong place. When the block's motion
//! cannot be simulated (an unsupported mode is in effect, or an unknown
//! code might move the machine) no move is emitted, and every axis is
//! reported as having an unknown position, so the interpreter does not draw
//! later moves from a stale starting point.

use dialect_api::{
    length_to_internal, literal, report_syntax_errors, ArcCenter, ArcDir, AxisOffset, AxisWord,
    BlockContext, CodeKey, Command, CoolantState, DistanceMode, FeedMode, ModalState, SpindleState,
};
use gcode_syntax::{Block, Number, Span};
use ir::{ArcPlane, DiagCode, Diagnostic, Diagnostics, LengthUnit, SourceRef, WorkOffsetId};

use crate::handler::Handler;
use crate::{DecimalInput, Fanuc};

const MOTION_GROUP: u16 = 1;
const PLANE_GROUP: u16 = 2;
const DISTANCE_GROUP: u16 = 3;
const UNITS_GROUP: u16 = 6;
const CANNED_CYCLE_GROUP: u16 = 9;

/// Key for the once-per-program decimal-point warning.
const DECIMAL_POINT_NOTE: &str = "fanuc.decimal_point";

/// Prefix of every "assumed default" warning message.
pub(crate) const ASSUMED: &str = "assumed ";

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
    /// The code uses the axis words itself (dwell time, offset data, its
    /// own move), so there is no ordinary move.
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

fn description(fanuc: &Fanuc, key: CodeKey) -> String {
    match fanuc.table.get(key).and_then(|d| d.description.as_deref()) {
        Some(desc) => format!("{key} ({desc})"),
        None => key.to_string(),
    }
}

pub(crate) fn interpret(
    fanuc: &Fanuc,
    block: &Block,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let src = ctx.src;
    if report_syntax_errors(block, src, diag) {
        return;
    }
    for a in &block.assignments {
        diag.warning(
            DiagCode::NotYetSimulated,
            "macro variable assignment is not simulated yet",
            at(src, a.span),
        );
    }

    // Validate the whole block before producing anything, so a rejected
    // block has no effect at all.
    let mut gcodes = Vec::new();
    let mut mcodes = Vec::new();
    let mut unknown = Vec::new();
    let mut values: Vec<Value> = Vec::new();
    for w in &block.words {
        let Some(number) = literal(&w.value) else {
            diag.error(
                DiagCode::NotYetSimulated,
                format!(
                    "{}: macro variables and expressions are not supported yet; block skipped",
                    w.letter
                ),
                at(src, w.span),
            );
            return;
        };
        match w.letter {
            'G' | 'M' => match CodeKey::from_value(w.letter, number.value)
                .and_then(|k| Some((k, fanuc.handler(k)?)))
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

    if !values_valid(&values, src, diag) {
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
        } else if state.first_report(&format!("fanuc.machine_m.{what}")) {
            // Not a Fanuc code, so the machine builder's (head selection,
            // clamps, vacuum, ...): noted once and ignored.
            diag.push(
                Diagnostic::info(
                    DiagCode::UnsupportedMCode,
                    format!(
                        "{what} is not a standard Fanuc code; it is taken to be \
                         machine-specific and ignored (no movement). If it moves the \
                         machine, list it in the machine file's ignore_codes or expect \
                         the drawing to differ. Not reported again."
                    ),
                    at(src, *span),
                )
                .dialect_specific(),
            );
        }
        out.push(Command::Unsupported { what });
    }

    let gcodes = resolve_group_conflicts(fanuc, gcodes, src, diag);
    if mcodes.len() > 1 && state.first_report("fanuc.multiple_m") {
        diag.push(
            Diagnostic::warning(
                DiagCode::ControllerDependent,
                "more than one M code in a block needs the multiple M-code option \
                 (parameter M3B); all are applied in order. Further cases are not reported.",
                at(src, mcodes.get(1).map_or(Span::default(), |c| c.span)),
            )
            .dialect_specific(),
        );
    }
    check_required_words(fanuc, gcodes.iter().chain(&mcodes), &values, src, diag);

    // 1. G-code mode changes: units first, so this block's values use them.
    for code in &gcodes {
        let unit = match code.handler {
            Handler::UnitsInch => LengthUnit::Inch,
            Handler::UnitsMetric => LengthUnit::Metric,
            _ => continue,
        };
        state.units = unit;
        set_active(fanuc, state, code.key);
        out.push(Command::SetUnits(unit));
    }
    let mut special = None;
    for code in &gcodes {
        let effect = apply_gcode(fanuc, code, ctx, state, out, diag);
        if matches!(
            code.handler,
            Handler::ReferenceReturn | Handler::ReferenceReturnOther | Handler::MachineRapid
        ) {
            special = Some(code);
        }
        motion_effect = match (motion_effect, effect) {
            (Motion::Consumed, _) | (_, Motion::Consumed) => Motion::Consumed,
            (Motion::CycleHoles, _) | (_, Motion::CycleHoles) => Motion::CycleHoles,
            _ => Motion::Normal,
        };
    }

    // 2. F, S, T.
    apply_values(&values, ctx.src, state, out, diag);

    // 3. M codes before motion.
    for code in mcodes.iter().filter(|c| c.handler.before_motion()) {
        apply_mcode(fanuc, code, &values, ctx, state, out, diag);
    }

    // 4. Motion.
    match special {
        Some(code) => special_motion(fanuc, code, &values, ctx, state, out, diag),
        None => motion(fanuc, &values, motion_effect, ctx, state, out, diag),
    }

    // 5. M codes after motion.
    for code in mcodes.iter().filter(|c| !c.handler.before_motion()) {
        apply_mcode(fanuc, code, &values, ctx, state, out, diag);
    }

    report_unused_words(fanuc, &gcodes, &mcodes, &values, ctx, state, diag);
}

/// Fanuc uses the last of several G codes from the same modal group in one
/// block. Earlier ones are dropped with a warning.
fn resolve_group_conflicts(
    fanuc: &Fanuc,
    gcodes: Vec<Code>,
    src: SourceRef,
    diag: &mut Diagnostics,
) -> Vec<Code> {
    let group_of = |k: CodeKey| fanuc.table.get(k).and_then(|d| d.group).filter(|&g| g != 0);
    let mut kept: Vec<Code> = Vec::new();
    for code in gcodes {
        if let Some(group) = group_of(code.key) {
            if let Some(pos) = kept.iter().position(|c| group_of(c.key) == Some(group)) {
                let earlier = kept.remove(pos);
                diag.push(
                    Diagnostic::warning(
                        DiagCode::ModalGroupConflict,
                        format!(
                            "{} and {} are in the same group; the last one ({}) is used",
                            earlier.key, code.key, code.key
                        ),
                        at(src, code.span),
                    )
                    .dialect_specific(),
                );
            }
        }
        kept.push(code);
    }
    kept
}

fn check_required_words<'a>(
    fanuc: &Fanuc,
    codes: impl Iterator<Item = &'a Code>,
    values: &[Value],
    src: SourceRef,
    diag: &mut Diagnostics,
) {
    for code in codes {
        let Some(def) = fanuc.table.get(code.key) else {
            continue;
        };
        for letter in &def.required_words {
            if !values.iter().any(|v| v.letter == *letter) {
                diag.error(
                    DiagCode::MissingWord,
                    format!("{} needs address {letter}", code.key),
                    at(src, code.span),
                );
            }
        }
    }
}

fn set_active(fanuc: &Fanuc, state: &mut ModalState, key: CodeKey) {
    if let Some(def) = fanuc.table.get(key) {
        if let (true, Some(group)) = (def.modal, def.group) {
            state.set_active(group, key);
        }
    }
}

/// Warns, once per program, that a mode the program never set is being
/// assumed.
fn assume(
    state: &mut ModalState,
    group: u16,
    code: &str,
    why: &str,
    src: SourceRef,
    diag: &mut Diagnostics,
) {
    if state.active_code(group).is_some() || !state.first_report(&format!("fanuc.assumed.{group}"))
    {
        return;
    }
    diag.push(
        Diagnostic::warning(
            DiagCode::ControllerDependent,
            format!(
                "{ASSUMED}{code}: the program has not selected this mode yet. {why} \
                 Add {code} near the start of the program to make it explicit."
            ),
            src,
        )
        .dialect_specific(),
    );
}

fn assume_units(state: &mut ModalState, src: SourceRef, diag: &mut Diagnostics) {
    assume(
        state,
        UNITS_GROUP,
        "G21 (mm)",
        "A Fanuc control keeps the units that were active when it was switched off, so the \
         machine may be in inches.",
        src,
        diag,
    );
}

/// The call an M98 block makes: `P` is the program, and `L` the repeat
/// count; without `L`, a `P` of more than four digits holds the repeat
/// count in its leading digits (`P190098`: 19 times program 98). Reports
/// and returns `None` if `P` is missing or not a whole number.
fn subprogram_call(values: &[Value], src: SourceRef, diag: &mut Diagnostics) -> Option<Command> {
    let whole = |v: &Value| -> Option<u32> {
        let n = v.number.value;
        (n >= 0.0 && n.fract() == 0.0 && n <= f64::from(u32::MAX)).then_some(n as u32)
    };
    let Some(p) = values.iter().find(|v| v.letter == 'P') else {
        diag.error(
            DiagCode::MissingWord,
            "M98 needs P (the program to call); no call is made",
            src,
        );
        return None;
    };
    let Some(p_value) = whole(p) else {
        diag.error(
            DiagCode::InvalidValue,
            format!(
                "M98 P{} is not a program number; no call is made",
                p.number.value
            ),
            at(src, p.span),
        );
        return None;
    };
    let l = values.iter().find(|v| v.letter == 'L');
    let (program, repeat) = match l {
        Some(l) => match whole(l) {
            Some(count) => (p_value, count),
            None => {
                diag.error(
                    DiagCode::InvalidValue,
                    format!(
                        "M98 L{} is not a repeat count; no call is made",
                        l.number.value
                    ),
                    at(src, l.span),
                );
                return None;
            }
        },
        None if p_value > 9999 => (p_value % 10_000, p_value / 10_000),
        None => (p_value, 1),
    };
    Some(Command::CallSubprogram { program, repeat })
}

/// Reports a code that changes how later blocks move but is not simulated
/// (owner decision, 2026-10-31: never hide moves): one warning, and later
/// moves are drawn without its effect.
fn ignored(fanuc: &Fanuc, code: &Code, src: SourceRef, diag: &mut Diagnostics) {
    diag.push(
        Diagnostic::warning(
            if code.key.letter() == 'M' {
                DiagCode::UnsupportedMCode
            } else {
                DiagCode::UnsupportedGCode
            },
            format!(
                "{} is not simulated and is ignored: its values are not a move, and later \
                 moves are drawn without its effect, so they may differ from the machine",
                description(fanuc, code.key)
            ),
            at(src, code.span),
        )
        .dialect_specific(),
    );
}

/// Applies one G code (other than units) and says how it affects the
/// block's motion.
fn apply_gcode(
    fanuc: &Fanuc,
    code: &Code,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) -> Motion {
    let src = at(ctx.src, code.span);
    let group = fanuc.table.get(code.key).and_then(|d| d.group);
    let plane_letters = match code.handler {
        Handler::PlaneXy => Some(('X', 'Y')),
        Handler::PlaneZx => Some(('Z', 'X')),
        Handler::PlaneYz => Some(('Y', 'Z')),
        _ => None,
    };
    if let Some((a, b)) = plane_letters {
        match ctx.axes.plane(a, b) {
            Some(plane) => {
                state.plane = Some(plane);
                set_active(fanuc, state, code.key);
                out.push(Command::SetPlane(plane));
            }
            None => diag.error(
                DiagCode::InvalidValue,
                format!(
                    "{} needs axes {a} and {b}, which this machine does not have",
                    code.key
                ),
                src,
            ),
        }
        return Motion::Normal;
    }
    match code.handler {
        Handler::Rapid | Handler::Linear | Handler::ArcCw | Handler::ArcCcw => {
            set_active(fanuc, state, code.key);
            // A group 01 code cancels a canned cycle, like G80.
            if let Some(g80) = CodeKey::parse("G80").filter(|k| fanuc.table.get(*k).is_some()) {
                state.set_active(CANNED_CYCLE_GROUP, g80);
            }
        }
        Handler::Absolute | Handler::Incremental => {
            let mode = if code.handler == Handler::Absolute {
                DistanceMode::Absolute
            } else {
                DistanceMode::Incremental
            };
            state.distance = mode;
            set_active(fanuc, state, code.key);
            out.push(Command::SetDistanceMode(mode));
        }
        Handler::WorkOffset => {
            // G54 is work offset 1, ..., G59 is 6. The cast saturates.
            let n = (code.key.value() - 53.0).round();
            let id = WorkOffsetId(n.clamp(0.0, f64::from(u16::MAX)) as u16);
            state.work_offset = id;
            set_active(fanuc, state, code.key);
            out.push(Command::SetWorkOffset(id));
        }
        Handler::FeedPerMinute => {
            state.feed_mode = FeedMode::PerMinute;
            set_active(fanuc, state, code.key);
            out.push(Command::SetFeedMode(FeedMode::PerMinute));
        }
        Handler::NoEffect => set_active(fanuc, state, code.key),
        Handler::NotSimulated => {
            set_active(fanuc, state, code.key);
            diag.warning(
                DiagCode::NotYetSimulated,
                format!(
                    "{} is not simulated yet; the path is shown without it",
                    description(fanuc, code.key)
                ),
                src,
            );
        }
        Handler::LengthCompensation => {
            set_active(fanuc, state, code.key);
            if state.first_report("fanuc.length_compensation") {
                diag.info(
                    DiagCode::NotYetSimulated,
                    format!(
                        "{} is not simulated: tool length (height) and wear compensation are \
                         ignored, and the path is shown as programmed. Not reported again.",
                        description(fanuc, code.key)
                    ),
                    src,
                );
            }
        }
        Handler::Ignored => {
            set_active(fanuc, state, code.key);
            ignored(fanuc, code, src, diag);
            out.push(Command::Unsupported {
                what: code.key.to_string(),
            });
            // Its words are data (an offset, a centre, an angle), not a move.
            return Motion::Consumed;
        }
        Handler::CannedCycle => {
            set_active(fanuc, state, code.key);
            if state.first_report(&format!("fanuc.cycle.{}", code.key)) {
                diag.push(
                    Diagnostic::warning(
                        DiagCode::UnsupportedGCode,
                        format!(
                            "{} is not simulated: its hole positions are drawn as rapids in the \
                             working plane; depths, R levels, pecks and retracts are not drawn. \
                             Not reported again.",
                            description(fanuc, code.key)
                        ),
                        at(src, code.span),
                    )
                    .dialect_specific(),
                );
            }
            out.push(Command::Unsupported {
                what: code.key.to_string(),
            });
            return Motion::CycleHoles;
        }
        Handler::Unsupported => {
            set_active(fanuc, state, code.key);
            let (effect, note) = match group {
                // One-shot codes (dwell) use the axis words as data.
                None | Some(0) => (Motion::Consumed, "; block not moved"),
                // Other modes (feed per revolution) leave the path alone.
                Some(_) => (Motion::Normal, "; the path is shown without it"),
            };
            diag.error(
                DiagCode::UnsupportedGCode,
                format!(
                    "{} is not supported yet{note}",
                    description(fanuc, code.key)
                ),
                src,
            );
            out.push(Command::Unsupported {
                what: code.key.to_string(),
            });
            return effect;
        }
        Handler::ReferenceReturn | Handler::ReferenceReturnOther | Handler::MachineRapid => {
            return Motion::Consumed;
        }
        // Units are applied earlier; M handlers never appear as G codes.
        _ => {}
    }
    Motion::Normal
}

/// Checks F, S and T values. Returns `false` (after reporting) if the
/// block must be skipped.
fn values_valid(values: &[Value], src: SourceRef, diag: &mut Diagnostics) -> bool {
    for v in values {
        let n = v.number.value;
        let problem = match v.letter {
            'F' if n <= 0.0 => "feed rate must be greater than zero",
            'S' if n < 0.0 => "spindle speed cannot be negative",
            'T' if v.number.decimal_point || n < 0.0 || n > f64::from(u32::MAX) => {
                "tool number must be a whole number"
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
                assume_units(state, at(src, v.span), diag);
                // Feed is never scaled by the least input increment.
                let feed = length_to_internal(n, state.units);
                state.feed_rate = Some(feed);
                out.push(Command::SetFeedRate(feed));
            }
            'S' => {
                state.spindle_rpm = n;
                out.push(Command::SetSpindleSpeed { spindle: 0, rpm: n });
            }
            'T' => {
                // Whole and in range (checked earlier), so the cast is exact.
                let tool = n as u32;
                state.selected_tool = Some(tool);
                out.push(Command::SelectTool(tool));
            }
            _ => {}
        }
    }
}

fn apply_mcode(
    fanuc: &Fanuc,
    code: &Code,
    values: &[Value],
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
        Handler::SpindleStop => spindle(out, state, SpindleState::Off),
        Handler::CoolantFlood => out.push(Command::Coolant(CoolantState::Flood)),
        Handler::CoolantOff => out.push(Command::Coolant(CoolantState::Off)),
        Handler::ToolChange => match state.selected_tool {
            Some(tool) => {
                state.current_tool = Some(tool);
                out.push(Command::ToolChange { tool });
            }
            // A warning: some machines use M6 alone for their own purposes,
            // such as a head change on a router.
            None => diag.warning(
                DiagCode::MissingWord,
                format!("{} with no tool selected (no T word yet)", code.key),
                src,
            ),
        },
        Handler::Stop => out.push(Command::Stop { optional: false }),
        Handler::OptionalStop => out.push(Command::Stop { optional: true }),
        Handler::ProgramEnd => out.push(Command::ProgramEnd { rewind: false }),
        Handler::ProgramEndRewind => out.push(Command::ProgramEnd { rewind: true }),
        Handler::Ignored => {
            ignored(fanuc, code, ctx.src, diag);
            out.push(Command::Unsupported {
                what: code.key.to_string(),
            });
        }
        Handler::SubprogramCall => {
            if let Some(call) = subprogram_call(values, src, diag) {
                out.push(call);
            }
        }
        Handler::SubprogramReturn => {
            if let Some(p) = values.iter().find(|v| v.letter == 'P') {
                diag.push(
                    Diagnostic::warning(
                        DiagCode::NotYetSimulated,
                        format!(
                            "{} P (return to sequence number {}) is not simulated: it returns \
                             to the block after the call",
                            code.key, p.number.value
                        ),
                        at(ctx.src, p.span),
                    )
                    .dialect_specific(),
                );
            }
            out.push(Command::ReturnFromSubprogram);
        }
        Handler::Unsupported => {
            diag.error(
                DiagCode::UnsupportedMCode,
                format!("{} is not supported yet", description(fanuc, code.key)),
                src,
            );
            out.push(Command::Unsupported {
                what: code.key.to_string(),
            });
        }
        _ => {}
    }
}

/// Converts a length or angle word to internal units, applying Fanuc's
/// decimal-point rule.
fn convert(
    fanuc: &Fanuc,
    v: &Value,
    rotary: bool,
    state: &mut ModalState,
    src: SourceRef,
    diag: &mut Diagnostics,
) -> f64 {
    if !rotary {
        assume_units(state, at(src, v.span), diag);
    }
    let n = v.number;
    let scaled = !n.decimal_point && fanuc.options.decimal_input == DecimalInput::LeastIncrement;
    let increment = match (rotary, state.units) {
        (true, _) | (false, LengthUnit::Metric) => 0.001,
        (false, LengthUnit::Inch) => 0.0001,
    };
    let value = if scaled { n.value * increment } else { n.value };
    if scaled && n.value != 0.0 && state.first_report(DECIMAL_POINT_NOTE) {
        let unit = match (rotary, state.units) {
            (true, _) => "deg",
            (false, LengthUnit::Metric) => "mm",
            (false, LengthUnit::Inch) => "inch",
        };
        diag.push(
            Diagnostic::warning(
                DiagCode::ControllerDependent,
                format!(
                    "{}{} has no decimal point, so it is read as {} {unit} (Fanuc default, \
                     parameter DPI = 0). Machines set to calculator-type input (DPI = 1) read \
                     it as {} {unit}. Further cases are not reported.",
                    v.letter, n.value, value, n.value
                ),
                at(src, v.span),
            )
            .dialect_specific(),
        );
    }
    if rotary {
        value
    } else {
        length_to_internal(value, state.units)
    }
}

/// The block's axis words as a target, in the current distance mode.
fn axis_target(
    fanuc: &Fanuc,
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
        let rotary = ctx.axes.axes().get(axis).is_some_and(|a| a.rotary);
        target.push(AxisWord {
            axis,
            value: convert(fanuc, v, rotary, state, ctx.src, diag),
            incremental,
        });
    }
    target
}

fn assume_distance(state: &mut ModalState, src: SourceRef, diag: &mut Diagnostics) {
    assume(
        state,
        DISTANCE_GROUP,
        "G90 (absolute)",
        "The power-on mode is set by a parameter (3402 bit 3).",
        src,
        diag,
    );
}

/// A canned cycle's hole: a rapid to the position the block gives in the
/// working plane. The other axes (the depth) and the cycle's own words (R,
/// Q, P) are not drawn.
fn cycle_hole(
    fanuc: &Fanuc,
    values: &[Value],
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let Some(plane) = state.plane else {
        return; // No working plane on this machine: nothing to draw.
    };
    assume(
        state,
        PLANE_GROUP,
        "G17 (XY plane)",
        "The power-on plane is set by a parameter (3402 bits 1 and 2).",
        ctx.src,
        diag,
    );
    assume_distance(state, ctx.src, diag);
    let incremental = state.distance == DistanceMode::Incremental;
    let in_plane = |a: usize| a == usize::from(plane.first) || a == usize::from(plane.second);
    let target: Vec<AxisWord> = axis_target(fanuc, values, incremental, ctx, state, diag)
        .into_iter()
        .filter(|w| in_plane(w.axis))
        .collect();
    if !target.is_empty() {
        out.push(Command::Rapid { target });
    }
}

/// G28, G30 and G53.
fn special_motion(
    fanuc: &Fanuc,
    code: &Code,
    values: &[Value],
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let src = at(ctx.src, code.span);
    if !values.iter().any(|v| ctx.axes.index_of(v.letter).is_some()) {
        return; // No axes named: nothing moves.
    }
    match code.handler {
        Handler::MachineRapid => {
            if state.distance == DistanceMode::Incremental {
                diag.push(
                    Diagnostic::warning(
                        DiagCode::ControllerDependent,
                        format!(
                            "{} is ignored in incremental mode (G91); block not moved",
                            code.key
                        ),
                        src,
                    )
                    .dialect_specific(),
                );
                return;
            }
            let target = axis_target(fanuc, values, false, ctx, state, diag);
            out.push(Command::MachineRapid { target });
        }
        Handler::ReferenceReturn | Handler::ReferenceReturnOther => {
            assume_distance(state, ctx.src, diag);
            let incremental = state.distance == DistanceMode::Incremental;
            let via = axis_target(fanuc, values, incremental, ctx, state, diag);
            // G28 goes to the 1st reference position; G30 to the 2nd, or
            // the one P2-P4 names.
            let position = if code.handler == Handler::ReferenceReturn {
                1
            } else {
                values
                    .iter()
                    .find(|v| v.letter == 'P')
                    .map_or(2.0, |p| p.number.value)
                    .clamp(2.0, 4.0) as u8
            };
            out.push(Command::ReturnToReference { via, position });
        }
        _ => {}
    }
}

/// The canned cycle in effect, if any (until G80 or G00-G03).
fn cycle_active(fanuc: &Fanuc, state: &ModalState) -> bool {
    state
        .active_code(CANNED_CYCLE_GROUP)
        .is_some_and(|k| fanuc.handler(k) == Some(Handler::CannedCycle))
}

fn motion(
    fanuc: &Fanuc,
    values: &[Value],
    effect: Motion,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) {
    let src = ctx.src;
    let mode = state
        .active_code(MOTION_GROUP)
        .and_then(|k| Some((k, fanuc.handler(k)?)));
    let is_arc = mode.is_some_and(|(_, h)| h.is_arc());

    let has_axes = values.iter().any(|v| ctx.axes.index_of(v.letter).is_some());
    let offset_values: Vec<(usize, &Value)> = values
        .iter()
        .filter_map(|v| Some((ctx.axes.index_of_arc_offset(v.letter)?, v)))
        .collect();
    let radius = values.iter().find(|v| v.letter == 'R');

    let wants_motion = has_axes || (is_arc && (!offset_values.is_empty() || radius.is_some()));
    if !wants_motion {
        return;
    }
    match effect {
        Motion::Consumed => return,
        Motion::CycleHoles => return cycle_hole(fanuc, values, ctx, state, out, diag),
        Motion::Normal if cycle_active(fanuc, state) => {
            return cycle_hole(fanuc, values, ctx, state, out, diag);
        }
        Motion::Normal => {}
    }

    let (mode_key, mode) = match mode {
        Some(m) => m,
        None => {
            // Power-on motion mode is a parameter (3402 bit 0); G00 is the
            // factory default.
            assume(
                state,
                MOTION_GROUP,
                "G00 (rapid)",
                "The power-on motion mode is set by a parameter (3402 bit 0).",
                src,
                diag,
            );
            let Some(g0) = CodeKey::parse("G00") else {
                return;
            };
            set_active(fanuc, state, g0);
            (g0, Handler::Rapid)
        }
    };
    if has_axes {
        assume_distance(state, src, diag);
    }
    let incremental = state.distance == DistanceMode::Incremental;
    let target = axis_target(fanuc, values, incremental, ctx, state, diag);

    let feed_ok = |state: &ModalState, diag: &mut Diagnostics| {
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
                    "no arc plane is available on this machine; block not moved",
                    src,
                );
                return;
            };
            assume(
                state,
                PLANE_GROUP,
                "G17 (XY plane)",
                "The power-on plane is set by a parameter (3402 bits 1 and 2).",
                src,
                diag,
            );
            let Some(center) = arc_center(
                fanuc,
                plane,
                radius,
                &offset_values,
                target.is_empty(),
                ctx,
                state,
                diag,
            ) else {
                return;
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

#[allow(clippy::too_many_arguments)]
fn arc_center(
    fanuc: &Fanuc,
    plane: ArcPlane,
    radius: Option<&Value>,
    offsets: &[(usize, &Value)],
    no_end_point: bool,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    diag: &mut Diagnostics,
) -> Option<ArcCenter> {
    let src = ctx.src;
    if let Some(r) = radius {
        if !offsets.is_empty() {
            diag.push(
                Diagnostic::warning(
                    DiagCode::ControllerDependent,
                    "both R and arc-centre offsets given; R is used and the offsets are ignored",
                    at(src, r.span),
                )
                .dialect_specific(),
            );
        }
        if no_end_point {
            diag.push(
                Diagnostic::warning(
                    DiagCode::InvalidArc,
                    "arc with R and no end point moves 0 degrees; block not moved",
                    at(src, r.span),
                )
                .dialect_specific(),
            );
            return None;
        }
        return Some(ArcCenter::Radius(convert(
            fanuc, r, false, state, src, diag,
        )));
    }
    if offsets.is_empty() {
        diag.error(
            DiagCode::InvalidArc,
            "arc needs R or centre offsets (e.g. I, J, K); block not moved",
            src,
        );
        return None;
    }
    let in_plane =
        |axis: usize| axis == usize::from(plane.first) || axis == usize::from(plane.second);
    let mut center = Vec::new();
    for (axis, v) in offsets {
        if in_plane(*axis) {
            let rotary = ctx.axes.axes().get(*axis).is_some_and(|a| a.rotary);
            center.push(AxisOffset {
                axis: *axis,
                value: convert(fanuc, v, rotary, state, src, diag),
            });
        } else {
            diag.warning(
                DiagCode::InvalidArc,
                format!("{} is not in the selected plane and is ignored", v.letter),
                at(src, v.span),
            );
        }
    }
    if center.is_empty() {
        diag.error(
            DiagCode::InvalidArc,
            "no centre offset in the selected plane; block not moved",
            src,
        );
        return None;
    }
    Some(ArcCenter::Offset(center))
}

/// Warns about words that nothing in the block used.
fn report_unused_words(
    fanuc: &Fanuc,
    gcodes: &[Code],
    mcodes: &[Code],
    values: &[Value],
    ctx: &BlockContext<'_>,
    state: &ModalState,
    diag: &mut Diagnostics,
) {
    let arc_mode = state
        .active_code(MOTION_GROUP)
        .and_then(|k| fanuc.handler(k))
        .is_some_and(Handler::is_arc);
    let declared = |letter: char| {
        gcodes.iter().chain(mcodes).any(|c| {
            fanuc.table.get(c.key).is_some_and(|d| {
                d.optional_words.contains(&letter) || d.required_words.contains(&letter)
            })
        })
    };
    for v in values {
        let used = matches!(v.letter, 'F' | 'S' | 'T' | 'O')
            || ctx.axes.index_of(v.letter).is_some()
            || (arc_mode && (v.letter == 'R' || ctx.axes.index_of_arc_offset(v.letter).is_some()))
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
