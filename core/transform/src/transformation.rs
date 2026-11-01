//! Working out a transformation's edits for a selection of lines (M4.6:
//! translate). The rules are in `docs/plans/M4_PLAN.md` §3.
//!
//! The program is run with a trace of the selected lines
//! ([`interpreter::interpret_traced`]). Walking the trace in run order, the
//! position the *rewritten* program will reach is followed block by block:
//!
//! - a selected move must end at its original end transformed: in G90 its
//!   axis words get the new end, in G91 the new end less where the
//!   rewritten program is before it; an axis it does not write gets a word
//!   only if it now has to change;
//! - the first move after the selection keeps its original end: if it is
//!   incremental it is adjusted the same way, so the rest of the program is
//!   unchanged; if it is absolute it is left as it is;
//! - blocks run in between that are not selected (a subprogram the
//!   selection calls) are replayed as they are written.

use std::collections::BTreeSet;
use std::ops::RangeInclusive;

use dialect_api::{ArcCenter, AxisWord, Command, Dialect, DistanceMode, ValueKind};
use gcode_syntax::{parse, Block, Expr, NumberStyle, Program as Parsed, Span, Word};
use interpreter::{interpret_traced, BlockTrace, Options};
use ir::{ArcPlane, LengthUnit, Pos, Severity, MAX_AXES};
use machine_api::{AxisKind, Machine};

use crate::{rewrite_word, source_lines, RewriteError, TextEdit};

/// What to do to the selected lines.
#[derive(Debug, Clone, PartialEq)]
pub enum Transformation {
    /// Move by `offset` (mm or degrees per machine axis, in program
    /// coordinates). Offsets on rotary axes must be zero.
    Translate {
        /// The offset, indexed like the machine's axes.
        offset: Pos,
    },
    /// Turn by `angle` degrees about `centre` in `plane`: counter-clockwise
    /// from the plane's first axis toward its second, as for arcs (so
    /// looking down the third axis for G17, G18 and G19). Axes outside the
    /// plane are unchanged. Arc centre offsets turn with the moves; R does
    /// not change.
    Rotate {
        /// Two linear axes of the machine (indices), in arc-plane order.
        plane: ArcPlane,
        /// Degrees, counter-clockwise.
        angle: f64,
        /// The point turned about (only the plane's two axes are used),
        /// in program coordinates.
        centre: Pos,
    },
}

/// Cosine and sine of `degrees`, exact for whole quarter turns, so 90°
/// gives 0 and not 6e-17.
fn cos_sin(degrees: f64) -> (f64, f64) {
    let turn = degrees.rem_euclid(360.0);
    if turn == 0.0 {
        (1.0, 0.0)
    } else if turn == 90.0 {
        (0.0, 1.0)
    } else if turn == 180.0 {
        (-1.0, 0.0)
    } else if turn == 270.0 {
        (0.0, -1.0)
    } else {
        let r = turn.to_radians();
        (r.cos(), r.sin())
    }
}

/// `v` turned counter-clockwise in `plane` (a vector: no centre).
fn turn(v: &Pos, plane: ArcPlane, degrees: f64) -> Pos {
    let (a, b) = (usize::from(plane.first), usize::from(plane.second));
    let (cos, sin) = cos_sin(degrees);
    let mut out = *v;
    if let (Some(&x), Some(&y)) = (v.get(a), v.get(b)) {
        if let Some(slot) = out.get_mut(a) {
            *slot = x * cos - y * sin;
        }
        if let Some(slot) = out.get_mut(b) {
            *slot = x * sin + y * cos;
        }
    }
    out
}

fn same_plane(a: ArcPlane, b: ArcPlane) -> bool {
    (a.first, a.second) == (b.first, b.second) || (a.first, a.second) == (b.second, b.first)
}

impl Transformation {
    /// Where a point of the selection goes.
    fn apply(&self, p: &Pos) -> Pos {
        match self {
            Self::Translate { offset } => {
                let mut out = *p;
                for (v, d) in out.iter_mut().zip(offset) {
                    *v += d;
                }
                out
            }
            Self::Rotate {
                plane,
                angle,
                centre,
            } => {
                let mut rel = *p;
                for (v, c) in rel.iter_mut().zip(centre) {
                    *v -= c;
                }
                let mut out = turn(&rel, *plane, *angle);
                for (v, c) in out.iter_mut().zip(centre) {
                    *v += c;
                }
                // Axes outside the plane are exactly as they were.
                for (i, (v, orig)) in out.iter_mut().zip(p).enumerate() {
                    if i != usize::from(plane.first) && i != usize::from(plane.second) {
                        *v = *orig;
                    }
                }
                out
            }
        }
    }

    /// What happens to a vector, such as an arc's centre offset from its
    /// start: a translation leaves it, a rotation turns it.
    fn apply_vector(&self, v: &Pos) -> Pos {
        match self {
            Self::Translate { .. } => *v,
            Self::Rotate { plane, angle, .. } => turn(v, *plane, *angle),
        }
    }

    /// Why an arc in `plane` cannot be transformed, if it cannot.
    fn arc_refusal(&self, plane: ArcPlane) -> Option<&'static str> {
        match self {
            Self::Translate { .. } => None,
            Self::Rotate {
                plane: turned,
                angle,
                ..
            } => (!same_plane(plane, *turned) && cos_sin(*angle) != (1.0, 0.0)).then_some(
                "the arc is not in the plane of the rotation, so turning it would take it out of its plane",
            ),
        }
    }
}

/// Something the user should know about a transformation that was done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// How much it matters.
    pub severity: Severity,
    /// The line it is about.
    pub line: u32,
    /// What to say.
    pub message: String,
}

/// The result of a transformation: the edits to make, and notes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transformed {
    /// The edits, ready for [`crate::apply_edits`].
    pub edits: Vec<TextEdit>,
    /// Warnings and notes, by line.
    pub notes: Vec<Note>,
}

/// Why a transformation was not done. Nothing is changed then.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransformError {
    /// Something in the selection (or the move after it) cannot be
    /// transformed exactly.
    #[error("line {line}: {reason}; nothing was changed")]
    Refused {
        /// The first line that cannot be transformed.
        line: u32,
        /// Why.
        reason: String,
    },
}

fn refuse(line: u32, reason: impl Into<String>) -> TransformError {
    TransformError::Refused {
        line,
        reason: reason.into(),
    }
}

/// Positions closer than this (mm or degrees) count as the same. Far
/// below the least input increment, far above rounding error.
const SAME: f64 = 1e-6;

/// Works out the edits that apply `transformation` to the lines in
/// `lines` (either order, both included) of `source`, run with `dialect`
/// on `machine`. Either every selected block can be transformed exactly,
/// or nothing is changed and the first line that cannot is named.
pub fn transform(
    source: &[u8],
    dialect: &dyn Dialect,
    machine: &dyn Machine,
    options: Options,
    lines: RangeInclusive<u32>,
    transformation: &Transformation,
) -> Result<Transformed, TransformError> {
    let (a, b) = (*lines.start(), *lines.end());
    let (first, last) = (a.min(b), a.max(b));
    check_transformation(transformation, machine, first)?;
    let plain = NumberStyle::of(b"0.").ok_or_else(|| refuse(first, "internal error"))?;
    if dialect
        .format_value(0.0, ValueKind::Length, LengthUnit::Metric, &plain)
        .is_none()
    {
        return Err(refuse(
            first,
            format!("the {} dialect cannot rewrite programs yet", dialect.id()),
        ));
    }
    let parsed = parse(source);
    let text_lines = source_lines(source);
    let total = u32::try_from(text_lines.len()).unwrap_or(u32::MAX);
    let job = Job {
        dialect,
        machine,
        parsed: &parsed,
        lines: &text_lines,
        first,
        last,
        transformation,
    };

    // Trace the selection and enough after it to find the next move. A
    // subprogram the selection calls may be anywhere: trace to the end.
    let mut extra: u32 = 64;
    loop {
        let end = last.saturating_add(extra);
        let (_, trace) =
            interpret_traced(source, "program", dialect, machine, options, first..=end);
        let calls = trace.iter().any(|t| {
            job.selected(t)
                && t.commands
                    .iter()
                    .any(|c| matches!(c, Command::CallSubprogram { .. }))
        });
        if calls && end < total {
            extra = total;
            continue;
        }
        match job.run(&trace) {
            Err(Incomplete) if end < total => extra = extra.saturating_mul(8),
            Err(Incomplete) => return job.finish(&trace, true),
            Ok(()) => return job.finish(&trace, false),
        }
    }
}

/// The move after the selection was not in the trace (yet).
struct Incomplete;

fn check_transformation(
    t: &Transformation,
    machine: &dyn Machine,
    line: u32,
) -> Result<(), TransformError> {
    match t {
        Transformation::Rotate {
            plane,
            angle,
            centre,
        } => {
            if !angle.is_finite() || centre.iter().any(|c| !c.is_finite()) {
                return Err(refuse(line, "the angle or the centre is not a number"));
            }
            let axes = machine.axes();
            let (a, b) = (usize::from(plane.first), usize::from(plane.second));
            match (axes.get(a), axes.get(b)) {
                (Some(x), Some(y))
                    if a != b && x.kind == AxisKind::Linear && y.kind == AxisKind::Linear =>
                {
                    Ok(())
                }
                (Some(_), Some(_)) if a != b => Err(refuse(line, "rotary axes cannot be rotated")),
                _ => Err(refuse(
                    line,
                    "the rotation plane must be two different axes of this machine",
                )),
            }
        }
        Transformation::Translate { offset } => {
            for (i, d) in offset.iter().enumerate() {
                if !d.is_finite() {
                    return Err(refuse(line, "the offset is not a number"));
                }
                if *d == 0.0 {
                    continue;
                }
                match machine.axes().get(i) {
                    None => {
                        return Err(refuse(
                            line,
                            "the offset is for an axis this machine does not have",
                        ))
                    }
                    Some(axis) if axis.kind == AxisKind::Rotary => {
                        return Err(refuse(
                            line,
                            format!("rotary axes ({}) cannot be translated", axis.name),
                        ))
                    }
                    Some(_) => {}
                }
            }
            Ok(())
        }
    }
}

struct Job<'a> {
    dialect: &'a dyn Dialect,
    machine: &'a dyn Machine,
    parsed: &'a Parsed,
    lines: &'a [&'a [u8]],
    first: u32,
    last: u32,
    transformation: &'a Transformation,
}

/// The block's motion command and the axis words it moves by.
fn motion(t: &BlockTrace) -> Option<(&Command, &[AxisWord])> {
    t.commands.iter().find_map(|c| match c {
        Command::Rapid { target } | Command::Linear { target } | Command::Arc { target, .. } => {
            Some((c, target.as_slice()))
        }
        _ => None,
    })
}

fn moves_position(t: &BlockTrace) -> bool {
    motion(t).is_some()
        || t.commands.iter().any(|c| {
            matches!(
                c,
                Command::MachineRapid { .. } | Command::ReturnToReference { .. }
            )
        })
}

fn far(a: &Pos, b: &Pos) -> bool {
    a.iter().zip(b).any(|(x, y)| (x - y).abs() > SAME)
}

impl Job<'_> {
    fn selected(&self, t: &BlockTrace) -> bool {
        (self.first..=self.last).contains(&t.line)
    }

    fn block(&self, t: &BlockTrace) -> Option<&Block> {
        self.parsed.blocks.get(t.block as usize)
    }

    /// Only checks that the trace reaches past the first pass over the
    /// selection to the next move (or the end of the program).
    fn run(&self, trace: &[BlockTrace]) -> Result<(), Incomplete> {
        let Some(end) = trace.iter().rposition(|t| self.selected(t) && t.run == 0) else {
            return Ok(());
        };
        let after = trace.get(end + 1..).unwrap_or_default();
        let found = after
            .iter()
            .take_while(|t| !self.selected(t))
            .any(moves_position);
        let next_pass = after.iter().any(|t| self.selected(t));
        let ended = after.iter().any(|t| {
            t.commands
                .iter()
                .any(|c| matches!(c, Command::ProgramEnd { .. }))
        });
        if found || next_pass || ended {
            Ok(())
        } else {
            Err(Incomplete)
        }
    }

    fn finish(&self, trace: &[BlockTrace], cut_short: bool) -> Result<Transformed, TransformError> {
        let mut out = Transformed::default();
        let selected: Vec<&BlockTrace> = trace.iter().filter(|t| self.selected(t)).collect();

        // Refusals first, over every run of every selected block.
        for t in &selected {
            self.check_block(t)?;
        }
        self.note_unrun(trace, &mut out);
        self.note_repeats(&selected, &mut out);

        let Some(start) = trace.iter().position(|t| self.selected(t)) else {
            return Ok(out);
        };
        let end = trace
            .iter()
            .rposition(|t| self.selected(t) && t.run == 0)
            .unwrap_or(start);
        let Some(first) = trace.get(start) else {
            return Ok(out);
        };
        let mut pos = first.before;
        let mut prev_after = first.before;
        let mut first_move = true;
        // Whether `pos` was reached by a move this transformation rewrote
        // (then it is the transformed point, rounded as written).
        let mut pos_transformed = false;
        let mut edited = BTreeSet::new();
        for t in trace.get(start..=end).unwrap_or_default() {
            if far(&t.before, &prev_after) {
                // Something not traced moved the tool (a subprogram far
                // down the file): assume it ends where it did.
                pos = t.before;
                pos_transformed = false;
            }
            prev_after = t.after;
            if self.selected(t) && t.run == 0 && edited.insert(t.block) {
                if let Some(call) = t.commands.iter().find_map(|c| match c {
                    Command::CallSubprogram { program, .. } => Some(*program),
                    _ => None,
                }) {
                    out.notes.push(Note {
                        severity: Severity::Warning,
                        line: t.line,
                        message: format!(
                            "line {} calls O{call}, which is not changed: its moves stay where they were",
                            t.line
                        ),
                    });
                }
                if motion(t).is_some() {
                    let target = self.transformation.apply(&t.after);
                    let start = self.transformation.apply(&t.before);
                    if std::mem::take(&mut first_move) && far(&pos, &start) {
                        out.notes.push(Note {
                            severity: Severity::Info,
                            line: t.line,
                            message: format!(
                                "line {}, the first move of the selection, still starts where it did, so its path \
                                 changes: check it in the view",
                                t.line
                            ),
                        });
                    }
                    // A start reached by a rewritten move is that point;
                    // otherwise it must be exactly the transformed start.
                    let check = (!pos_transformed).then_some(&start);
                    pos = self.rewrite(t, &pos, check, &target, true, &mut out.edits)?;
                    pos_transformed = true;
                }
            } else if moves_position(t) {
                pos = self.replay(t, &pos);
                pos_transformed = false;
            }
        }

        // The first move after the selection.
        let after = trace
            .get(end + 1..)
            .unwrap_or_default()
            .iter()
            .take_while(|t| !self.selected(t))
            .find(|t| moves_position(t));
        let last_state = trace.get(end).map(|t| t.state.distance);
        match after {
            Some(t) if far(&pos, &t.before) => self.after_selection(t, &pos, &mut out)?,
            Some(_) => {}
            None if cut_short && last_state == Some(DistanceMode::Incremental) => out.notes.push(Note {
                severity: Severity::Warning,
                line: self.last,
                message: "the move after the selection could not be found, so it was not adjusted: check \
                          the rest of the program"
                    .to_owned(),
            }),
            None => {}
        }
        Ok(out)
    }

    fn after_selection(
        &self,
        t: &BlockTrace,
        pos: &Pos,
        out: &mut Transformed,
    ) -> Result<(), TransformError> {
        if let Some(why) = self
            .block(t)
            .and_then(|b| self.dialect.transform_refusal(b, &t.state))
        {
            out.notes.push(Note {
                severity: Severity::Info,
                line: t.line,
                message: format!(
                    "line {}, after the selection, now starts from the moved point ({why}): check it in the view",
                    t.line
                ),
            });
            return Ok(());
        }
        let Some((command, _)) = motion(t) else {
            return Ok(());
        };
        if t.errors || !t.known_before {
            return Err(refuse(
                t.line,
                "the move after the selection cannot be adjusted",
            ));
        }
        if matches!(command, Command::Arc { .. }) {
            return Err(refuse(
                t.line,
                "the move after the selection is an arc, and its start would move but not its end, so it would \
                 no longer be the same arc; end the selection after this arc, or before the move into it",
            ));
        }
        if t.state.distance == DistanceMode::Incremental {
            let mut edits = Vec::new();
            self.rewrite(t, pos, None, &t.after, false, &mut edits)?;
            if !edits.is_empty() {
                out.edits.extend(edits);
                out.notes.push(Note {
                    severity: Severity::Warning,
                    line: t.line,
                    message: format!(
                        "line {}, after the selection, was changed too, so that the rest of the program stays where \
                         it was",
                        t.line
                    ),
                });
            }
        } else {
            out.notes.push(Note {
                severity: Severity::Info,
                line: t.line,
                message: format!(
                    "line {}, after the selection, now starts from the moved point (axes it does not write stay \
                     moved): check it in the view",
                    t.line
                ),
            });
        }
        Ok(())
    }

    fn check_block(&self, t: &BlockTrace) -> Result<(), TransformError> {
        let block = self
            .block(t)
            .ok_or_else(|| refuse(t.line, "internal error: no such block"))?;
        if let Some(why) = self.dialect.transform_refusal(block, &t.state) {
            return Err(refuse(t.line, why));
        }
        if t.errors {
            return Err(refuse(
                t.line,
                "the line has an error (see the diagnostics), so it may not run as written",
            ));
        }
        for c in &t.commands {
            match c {
                Command::MachineRapid { .. } => {
                    return Err(refuse(t.line, "the line moves in machine coordinates"))
                }
                Command::ReturnToReference { .. } => {
                    return Err(refuse(t.line, "the line returns to a reference position"))
                }
                Command::PositionUnknown { .. } => {
                    return Err(refuse(t.line, "the line leaves the position unknown"))
                }
                _ => {}
            }
        }
        for c in &t.commands {
            if let Command::Arc { plane, .. } = c {
                if let Some(why) = self.transformation.arc_refusal(*plane) {
                    return Err(refuse(t.line, why));
                }
            }
        }
        if !t.known_before || !t.known_after {
            return Err(refuse(
                t.line,
                "the position here is unknown, so it cannot be moved",
            ));
        }
        for w in &block.words {
            if !matches!(w.value, Expr::Number(_)) {
                return Err(refuse(
                    t.line,
                    format!("{} is a variable or expression", w.letter),
                ));
            }
        }
        Ok(())
    }

    fn note_unrun(&self, trace: &[BlockTrace], out: &mut Transformed) {
        let ran: BTreeSet<u32> = trace.iter().map(|t| t.block).collect();
        let unrun: Vec<u32> = self
            .parsed
            .blocks
            .iter()
            .enumerate()
            .filter(|(i, b)| {
                (self.first..=self.last).contains(&b.line)
                    && !b.words.is_empty()
                    && !ran.contains(&u32::try_from(*i).unwrap_or(u32::MAX))
            })
            .map(|(_, b)| b.line)
            .collect();
        if let Some(&line) = unrun.first() {
            let more = unrun.len() - 1;
            out.notes.push(Note {
                severity: Severity::Info,
                line,
                message: if more == 0 {
                    format!("line {line} is never run, so it was not changed")
                } else {
                    format!("line {line} and {more} more selected lines are never run, so they were not changed")
                },
            });
        }
    }

    fn note_repeats(&self, selected: &[&BlockTrace], out: &mut Transformed) {
        let runs = selected.iter().map(|t| t.run).max().unwrap_or(0);
        if runs > 0 {
            if let Some(t) = selected.iter().find(|t| t.run == runs) {
                out.notes.push(Note {
                    severity: Severity::Warning,
                    line: t.line,
                    message: format!(
                        "selected lines run {} times (a subprogram); the text is changed once, which changes every \
                         run",
                        runs + 1
                    ),
                });
            }
        }
    }

    /// Where a block that is not rewritten leaves the tool in the rewritten
    /// program: the axes it writes absolutely where it put them, the axes
    /// it moves incrementally by the same amount, the others unchanged.
    fn replay(&self, t: &BlockTrace, pos: &Pos) -> Pos {
        let mut next = *pos;
        let words: Vec<&AxisWord> = t
            .commands
            .iter()
            .flat_map(|c| match c {
                Command::Rapid { target }
                | Command::Linear { target }
                | Command::Arc { target, .. } => target.iter().collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect();
        for w in words {
            if let (Some(slot), Some(a), Some(b)) = (
                next.get_mut(w.axis),
                t.after.get(w.axis),
                t.before.get(w.axis),
            ) {
                *slot = if w.incremental { *slot + (a - b) } else { *a };
            }
        }
        next
    }

    /// Rewrites the axis words of `t`'s move so that, starting from `pos`
    /// in the rewritten program, it ends at `target`. `start`, if given, is
    /// where the move must start for an arc to stay the same arc (`None`
    /// when `pos` was reached by a rewritten move, so it is that point). Returns where the
    /// rewritten move ends (the written values, as the dialect reads them).
    fn rewrite(
        &self,
        t: &BlockTrace,
        pos: &Pos,
        start: Option<&Pos>,
        target: &Pos,
        turn_offsets: bool,
        edits: &mut Vec<TextEdit>,
    ) -> Result<Pos, TransformError> {
        let block = self
            .block(t)
            .ok_or_else(|| refuse(t.line, "internal error: no such block"))?;
        let line_text: &[u8] = self
            .lines
            .get((t.line as usize).saturating_sub(1))
            .ok_or_else(|| refuse(t.line, "internal error: no such line"))?;
        // An arc stays the same arc only if its start moved with it: the
        // point a rewritten move reached (rounded as written), or exactly
        // the transformed start.
        if let Some((Command::Arc { .. }, _)) = motion(t) {
            if start.is_some_and(|s| far(pos, s)) {
                return Err(refuse(
                    t.line,
                    "the selection starts with an arc, whose start point is not moved, so it would no longer be \
                     the same arc; start the selection at the move before it",
                ));
            }
        }
        let units = t.state.units;
        let incremental = t.state.distance == DistanceMode::Incremental;
        let mut reached = *pos;
        let mut insert: Vec<(usize, char, ValueKind, f64)> = Vec::new();
        for (i, axis) in self.machine.axes().iter().enumerate().take(MAX_AXES) {
            let (Some(&goal), Some(&from)) = (target.get(i), pos.get(i)) else {
                continue;
            };
            let kind = match axis.kind {
                AxisKind::Linear => ValueKind::Length,
                AxisKind::Rotary => ValueKind::Angle,
            };
            let value = if incremental { goal - from } else { goal };
            let words: Vec<&Word> = block
                .words
                .iter()
                .filter(|w| w.letter == axis.name)
                .collect();
            match words.as_slice() {
                [] if (goal - from).abs() <= SAME => {}
                [] => insert.push((i, axis.name, kind, value)),
                [word] => {
                    let edit =
                        rewrite_word(line_text, t.line, word, value, kind, units, self.dialect)
                            .map_err(|e| self.rewrite_refusal(e))?;
                    let written = match &edit {
                        Some(e) => self.read_back(axis.name, &e.text, kind, units, t.line)?,
                        None => match &word.value {
                            Expr::Number(n) => self
                                .dialect
                                .read_value(n, kind, units)
                                .ok_or_else(|| refuse(t.line, "the value cannot be read"))?,
                            _ => {
                                return Err(refuse(
                                    t.line,
                                    format!("{} is not a plain number", axis.name),
                                ))
                            }
                        },
                    };
                    if let Some(slot) = reached.get_mut(i) {
                        *slot = if incremental { from + written } else { written };
                    }
                    edits.extend(edit);
                }
                _ => {
                    return Err(refuse(
                        t.line,
                        format!("{} is written twice in the line", axis.name),
                    ))
                }
            }
        }
        let mut insert_offsets: Vec<(char, f64)> = Vec::new();
        if let (true, Some((Command::Arc { center, plane, .. }, _))) = (turn_offsets, motion(t)) {
            self.turn_centre(
                t,
                block,
                line_text,
                center,
                *plane,
                edits,
                &mut insert_offsets,
            )?;
        }
        if !insert.is_empty() || !insert_offsets.is_empty() {
            let axis_letters: Vec<char> = self.machine.axes().iter().map(|a| a.name).collect();
            let (at, prefix, upper, style) =
                self.insertion_point(block, line_text, &axis_letters)?;
            for (i, letter, kind, value) in insert {
                let text = self
                    .dialect
                    .format_value(value, kind, units, &style)
                    .ok_or_else(|| {
                        refuse(t.line, format!("the new {letter} value cannot be written"))
                    })?;
                let letter = if upper {
                    letter
                } else {
                    letter.to_ascii_lowercase()
                };
                let written =
                    self.read_back(letter.to_ascii_uppercase(), &text, kind, units, t.line)?;
                if let (Some(slot), Some(&from)) = (reached.get_mut(i), pos.get(i)) {
                    *slot = if incremental { from + written } else { written };
                }
                edits.push(TextEdit {
                    line: t.line,
                    span: Span::new(at, at),
                    text: format!("{prefix}{letter}{text}"),
                });
            }
            // Centre offsets after the block's offset words, or after its
            // axis words (and any just added there).
            let offset_letters: Vec<char> = self
                .machine
                .axes()
                .iter()
                .filter_map(|a| a.arc_offset_letter)
                .collect();
            let has_offset_word = block
                .words
                .iter()
                .any(|w| offset_letters.contains(&w.letter));
            let (at, prefix, upper, style) = if has_offset_word {
                self.insertion_point(block, line_text, &offset_letters)?
            } else {
                (at, prefix, upper, style)
            };
            for (letter, value) in insert_offsets {
                let text = self
                    .dialect
                    .format_value(value, ValueKind::Length, units, &style)
                    .ok_or_else(|| {
                        refuse(t.line, format!("the new {letter} value cannot be written"))
                    })?;
                let letter = if upper {
                    letter
                } else {
                    letter.to_ascii_lowercase()
                };
                edits.push(TextEdit {
                    line: t.line,
                    span: Span::new(at, at),
                    text: format!("{prefix}{letter}{text}"),
                });
            }
        }
        Ok(reached)
    }

    /// Rewrites an arc's centre offsets (I/J/K, letters from the machine
    /// file) as the transformation turns them; offsets that were not
    /// written but are no longer zero go to `insert`. R is left as it is.
    #[allow(clippy::too_many_arguments)]
    fn turn_centre(
        &self,
        t: &BlockTrace,
        block: &Block,
        line_text: &[u8],
        center: &ArcCenter,
        plane: ArcPlane,
        edits: &mut Vec<TextEdit>,
        insert: &mut Vec<(char, f64)>,
    ) -> Result<(), TransformError> {
        let offsets = match center {
            ArcCenter::Radius(_) => return Ok(()),
            ArcCenter::Offset(offsets) => offsets,
            ArcCenter::Position(_) => {
                return Err(refuse(
                    t.line,
                    "an arc centre given as a position cannot be transformed yet",
                ))
            }
        };
        let mut old: Pos = [0.0; MAX_AXES];
        for o in offsets {
            if let Some(slot) = old.get_mut(o.axis) {
                *slot = o.value;
            }
        }
        let new = self.transformation.apply_vector(&old);
        let units = t.state.units;
        for axis in [usize::from(plane.first), usize::from(plane.second)] {
            let (Some(&was), Some(&now)) = (old.get(axis), new.get(axis)) else {
                continue;
            };
            let letter = self
                .machine
                .axes()
                .get(axis)
                .and_then(|a| a.arc_offset_letter);
            let words: Vec<&Word> = letter
                .map(|l| block.words.iter().filter(|w| w.letter == l).collect())
                .unwrap_or_default();
            match (letter, words.as_slice()) {
                (_, []) if (now - was).abs() <= SAME => {}
                (Some(l), []) => insert.push((l, now)),
                (None, _) => {
                    return Err(refuse(
                        t.line,
                        "the machine file gives this axis no arc-centre letter",
                    ))
                }
                (Some(_), [word]) => {
                    let edit = rewrite_word(
                        line_text,
                        t.line,
                        word,
                        now,
                        ValueKind::Length,
                        units,
                        self.dialect,
                    )
                    .map_err(|e| self.rewrite_refusal(e))?;
                    edits.extend(edit);
                }
                (Some(l), _) => {
                    return Err(refuse(t.line, format!("{l} is written twice in the line")))
                }
            }
        }
        Ok(())
    }

    /// Where to add words to a block: after its last word with one of
    /// `letters` (or its last word), with a space if the line's words are
    /// spaced, in the letter case and number style of that word.
    fn insertion_point(
        &self,
        block: &Block,
        line: &[u8],
        letters: &[char],
    ) -> Result<(usize, &'static str, bool, NumberStyle), TransformError> {
        let is_axis = |w: &&Word| letters.contains(&w.letter);
        let anchor = block
            .words
            .iter()
            .rfind(is_axis)
            .or_else(|| block.words.last())
            .ok_or_else(|| refuse(block.line, "there is no word to add a value after"))?;
        // Spaced like the line's other words; a line of one word gets a
        // space, the common style.
        let spaced = block.words.len() < 2
            || block.words.iter().skip(1).any(|w| {
                w.span
                    .start
                    .checked_sub(1)
                    .and_then(|i| line.get(i))
                    .is_some_and(u8::is_ascii_whitespace)
            });
        let upper = line
            .get(anchor.span.start)
            .is_none_or(u8::is_ascii_uppercase);
        let style = anchor
            .value_span(line)
            .and_then(|s| line.get(s.start..s.end))
            .and_then(NumberStyle::of)
            .filter(|s| is_axis(&anchor) || s.decimal_point)
            .map(|s| NumberStyle {
                negative: false,
                plus: false,
                sign_gap: String::new(),
                ..s
            })
            .or_else(|| NumberStyle::of(b"0."))
            .ok_or_else(|| refuse(block.line, "internal error"))?;
        Ok((anchor.span.end, if spaced { " " } else { "" }, upper, style))
    }

    fn read_back(
        &self,
        letter: char,
        text: &str,
        kind: ValueKind,
        units: LengthUnit,
        line: u32,
    ) -> Result<f64, TransformError> {
        let reparsed = parse(format!("{letter}{text}").as_bytes());
        match reparsed
            .blocks
            .first()
            .and_then(|b| b.words.first())
            .map(|w| &w.value)
        {
            Some(Expr::Number(n)) => self
                .dialect
                .read_value(n, kind, units)
                .ok_or_else(|| refuse(line, "the new value cannot be read back")),
            _ => Err(refuse(line, "the new value cannot be read back")),
        }
    }

    fn rewrite_refusal(&self, e: RewriteError) -> TransformError {
        match e {
            RewriteError::NotANumber { line, letter } => {
                refuse(line, format!("{letter} is not a plain number"))
            }
            RewriteError::CannotWrite { line, letter } => refuse(
                line,
                format!("the new {letter} value cannot be written for this control"),
            ),
        }
    }
}
