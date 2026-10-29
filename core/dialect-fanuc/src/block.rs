//! Interpretation of one block.
//!
//! Order of effects within a block (see `docs/dialect-notes/fanuc.md`):
//! 1. G-code mode changes (units first, then the rest in written order),
//! 2. F, S and T values,
//! 3. M codes that act before motion (M3, M4, M6, M8),
//! 4. the motion itself,
//! 5. M codes that act after motion (M0, M1, M2, M5, M9, M30).

use dialect_api::{
    length_to_internal, literal, report_syntax_errors, ArcCenter, ArcDir, AxisOffset, AxisWord,
    BlockContext, CodeKey, Command, CoolantState, DistanceMode, FeedMode, ModalState, SpindleState,
};
use gcode_syntax::{Block, Number, Span};
use ir::{ArcPlane, DiagCode, Diagnostic, Diagnostics, LengthUnit, SourceRef, WorkOffsetId};

use crate::handler::Handler;
use crate::{DecimalInput, Fanuc};

const MOTION_GROUP: u16 = 1;
const CANNED_CYCLE_GROUP: u16 = 9;

/// Key for the once-per-program decimal-point warning.
const DECIMAL_POINT_NOTE: &str = "fanuc.decimal_point";

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

    // Unknown codes. An unknown G code might change motion, so the block
    // does not move.
    let mut skip_motion = false;
    for (letter, number, span) in &unknown {
        let what = format!("{letter}{}", number.value);
        let (code, note) = if *letter == 'G' {
            skip_motion = true;
            (DiagCode::UnsupportedGCode, "; block not moved")
        } else {
            (DiagCode::UnsupportedMCode, "")
        };
        diag.error(
            code,
            format!("{what} is not supported{note}"),
            at(src, *span),
        );
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
    for code in &gcodes {
        skip_motion |= apply_gcode(fanuc, code, ctx, state, out, diag);
    }

    // 2. F, S, T.
    apply_values(&values, state, out);

    // 3. M codes before motion.
    for code in mcodes.iter().filter(|c| c.handler.before_motion()) {
        apply_mcode(fanuc, code, state, out, src, diag);
    }

    // 4. Motion.
    motion(fanuc, &values, skip_motion, ctx, state, out, diag);

    // 5. M codes after motion.
    for code in mcodes.iter().filter(|c| !c.handler.before_motion()) {
        apply_mcode(fanuc, code, state, out, src, diag);
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

/// Applies one G code (other than units). Returns `true` if the block must
/// not move because of it.
fn apply_gcode(
    fanuc: &Fanuc,
    code: &Code,
    ctx: &BlockContext<'_>,
    state: &mut ModalState,
    out: &mut Vec<Command>,
    diag: &mut Diagnostics,
) -> bool {
    let src = at(ctx.src, code.span);
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
        return false;
    }
    match code.handler {
        Handler::Rapid | Handler::Linear | Handler::ArcCw | Handler::ArcCcw => {
            set_active(fanuc, state, code.key);
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
        Handler::Unsupported => {
            set_active(fanuc, state, code.key);
            let group = fanuc.table.get(code.key).and_then(|d| d.group);
            // One-shot codes and canned cycles change what the axis words
            // mean, so the block must not be shown as a plain move.
            let blocks_motion = matches!(group, None | Some(0) | Some(CANNED_CYCLE_GROUP));
            let note = if blocks_motion {
                "; block not moved"
            } else {
                ""
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
            return blocks_motion;
        }
        // Units are applied earlier; M handlers never appear as G codes.
        _ => {}
    }
    false
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
fn apply_values(values: &[Value], state: &mut ModalState, out: &mut Vec<Command>) {
    for v in values {
        let n = v.number.value;
        match v.letter {
            'F' => {
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
    state: &mut ModalState,
    out: &mut Vec<Command>,
    src: SourceRef,
    diag: &mut Diagnostics,
) {
    let src = at(src, code.span);
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

fn motion(
    fanuc: &Fanuc,
    values: &[Value],
    skip_motion: bool,
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

    let axis_values: Vec<(usize, &Value)> = values
        .iter()
        .filter_map(|v| Some((ctx.axes.index_of(v.letter)?, v)))
        .collect();
    let offset_values: Vec<(usize, &Value)> = values
        .iter()
        .filter_map(|v| Some((ctx.axes.index_of_arc_offset(v.letter)?, v)))
        .collect();
    let radius = values.iter().find(|v| v.letter == 'R');

    let wants_motion =
        !axis_values.is_empty() || (is_arc && (!offset_values.is_empty() || radius.is_some()));
    if !wants_motion || skip_motion {
        return;
    }
    if let Some(cycle) = state
        .active_code(CANNED_CYCLE_GROUP)
        .filter(|&k| fanuc.handler(k) == Some(Handler::Unsupported))
    {
        diag.error(
            DiagCode::UnsupportedGCode,
            format!("canned cycle {cycle} is active and not supported yet; block not moved"),
            src,
        );
        return;
    }
    let Some((mode_key, mode)) = mode else {
        return;
    };

    let incremental = state.distance == DistanceMode::Incremental;
    let mut target = Vec::with_capacity(axis_values.len());
    for (axis, v) in &axis_values {
        let rotary = ctx.axes.axes().get(*axis).is_some_and(|a| a.rotary);
        target.push(AxisWord {
            axis: *axis,
            value: convert(fanuc, v, rotary, state, src, diag),
            incremental,
        });
    }

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
