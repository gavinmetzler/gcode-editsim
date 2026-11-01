//! Interpreter: runs a program through a dialect and a machine model to
//! produce the IR.
//!
//! [`interpret`] parses the source, lets the [`Dialect`] turn each block
//! into commands, and applies those commands to a [`Machine`]: it tracks
//! the machine position, applies work offsets, works out arc centres,
//! checks travel limits and emits [`ir::Segment`]s in program order.
//!
//! Positions in the output are **machine coordinates** (work offsets
//! already applied), in internal units. The machine starts at its reference
//! position (machine coordinate zero on axes whose definition gives none).
//!
//! A move is only drawn if the position it starts from is known. After the
//! dialect reports that the machine moved somewhere it cannot model
//! ([`Command::PositionUnknown`]), moves are left out, with one warning,
//! until every axis has been given an absolute position again.

#![forbid(unsafe_code)]

mod arc;

use std::collections::BTreeSet;

use dialect_api::{
    literal, ArcCenter, ArcDir, AxisInfo, AxisSet, AxisWord, BlockContext, CodeKey, Command,
    CoolantState, Dialect, FeedMode, ModalState, SpindleState,
};
use gcode_syntax::Block;
use ir::{
    bounding_box, path_length, ArcGeom, ArcPlane, ChannelStream, CycleStep, DiagCode, Diagnostic,
    Diagnostics, LengthUnit, Pos, Program, ProgramMeta, SegFlags, SegKind, Segment, SourceRef,
    WorkOffsetId, MAX_AXES,
};
use machine_api::{AxisKind, LimitViolation, Machine};

use crate::arc::{ArcProblem, RADIUS_TOLERANCE, SAME_POINT};

/// Settings for one interpretation run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// The operator's block-delete switch. When `true`, blocks starting
    /// with `/` are skipped; when `false` (the default) they run.
    pub block_delete: bool,
    /// The most blocks to run, counting each run of a subprogram block, so
    /// that repeat counts and calls cannot run away. The default is
    /// [`DEFAULT_MAX_BLOCKS`].
    pub max_blocks: usize,
}

/// The default block limit: far beyond real programs, small enough to stop
/// a runaway program in seconds.
pub const DEFAULT_MAX_BLOCKS: usize = 2_000_000;

impl Default for Options {
    fn default() -> Self {
        Self {
            block_delete: false,
            max_blocks: DEFAULT_MAX_BLOCKS,
        }
    }
}

/// How deep subprogram calls may nest. Controls allow a few levels (Fanuc
/// documents four for M98, more with macro calls); this is generous and
/// stops a program that calls itself.
pub const MAX_CALL_DEPTH: usize = 10;

/// A subprogram being run: where to return to, where it starts, and how
/// many more runs are left after this one.
struct Frame {
    return_to: usize,
    start: usize,
    program: u32,
    remaining: u32,
}

/// Out-of-limit moves reported individually before the rest are only
/// flagged on their segments.
const MAX_LIMIT_REPORTS: usize = 50;

/// Interprets a whole program.
///
/// Never fails and never panics: every problem becomes a diagnostic in the
/// returned [`Program`], and interpretation continues with the next block.
pub fn interpret(
    source: &[u8],
    file_name: &str,
    dialect: &dyn Dialect,
    machine: &dyn Machine,
    options: Options,
) -> Program {
    let mut diag = Diagnostics::new();
    let axes = match axis_set(machine) {
        Ok(axes) => axes,
        Err(message) => {
            diag.error(DiagCode::InvalidValue, message, SourceRef::default());
            return finish(Vec::new(), diag, file_name, ProgramMeta::default());
        }
    };
    let parsed = gcode_syntax::parse(source);
    let ignored: BTreeSet<CodeKey> = machine
        .ignored_codes()
        .iter()
        .filter_map(|c| CodeKey::parse(c))
        .collect();
    let mut noted_ignored = BTreeSet::new();
    let mut exec = Executor::new(dialect.initial_state(&axes), machine);
    let mut commands = Vec::new();

    // Where each program in the file starts (its number block), and which
    // program is the main one: the number on the first block that does
    // anything, if it has one.
    let blocks = &parsed.blocks;
    let mut programs = std::collections::BTreeMap::new();
    for (index, block) in blocks.iter().enumerate() {
        if let Some(n) = dialect.program_number(block) {
            programs.entry(n).or_insert(index);
        }
    }
    let main_header = blocks.iter().position(has_content).filter(|&i| {
        blocks
            .get(i)
            .is_some_and(|b| dialect.program_number(b).is_some())
    });

    let mut stack: Vec<Frame> = Vec::new();
    let mut ran = vec![false; blocks.len()];
    let mut runs = 0usize;
    let mut pc = 0usize;
    let src_of = |index: usize, block: &Block| SourceRef {
        file: 0,
        line: block.line,
        block: u32::try_from(index).unwrap_or(u32::MAX),
        col_start: 0,
        col_end: 0,
    };

    while !exec.ended {
        let Some(block) = blocks.get(pc) else {
            // The end of the file inside a subprogram: return from it.
            if stack.is_empty() {
                break;
            }
            let program = stack.last().map_or(0, |f| f.program);
            diag.warning(
                DiagCode::NotYetSimulated,
                format!("O{program} reaches the end of the file without M99: it returns there"),
                blocks
                    .last()
                    .map_or_else(SourceRef::default, |b| src_of(blocks.len() - 1, b)),
            );
            pc = return_from(&mut stack);
            continue;
        };
        let src = src_of(pc, block);

        // Running on into the start of another program ends the current
        // one (a call jumps past the number block, so only running on gets
        // here).
        if let Some(n) = dialect.program_number(block) {
            if Some(pc) != main_header {
                if stack.is_empty() {
                    diag.info(
                        DiagCode::NotYetSimulated,
                        format!(
                            "the main program runs on into program O{n} without M30 or M02; \
                             it ends here"
                        ),
                        src,
                    );
                    break;
                }
                diag.warning(
                    DiagCode::NotYetSimulated,
                    format!(
                        "O{} runs on into program O{n} without M99: it returns here",
                        stack.last().map_or(0, |f| f.program)
                    ),
                    src,
                );
                pc = return_from(&mut stack);
                continue;
            }
        }

        runs += 1;
        if runs > options.max_blocks {
            diag.error(
                DiagCode::NotYetSimulated,
                format!(
                    "the program ran more than {} blocks (subprogram calls and repeats); it is \
                     stopped here",
                    options.max_blocks
                ),
                src,
            );
            break;
        }
        if let Some(r) = ran.get_mut(pc) {
            *r = true;
        }
        let mut next = pc + 1;
        if !(options.block_delete && block.block_skip.is_some()) {
            commands.clear();
            let stripped = strip_ignored(block, &ignored, &mut noted_ignored, src, &mut diag);
            let block = stripped.as_ref().unwrap_or(block);
            let ctx = BlockContext { axes: &axes, src };
            dialect.interpret_block(block, &ctx, &mut exec.state, &mut commands, &mut diag);
            for command in &commands {
                exec.apply(command, src, &mut diag);
            }
            // Calls and returns take effect after the block's other commands.
            for command in &commands {
                match command {
                    Command::CallSubprogram { program, repeat } if *repeat > 0 => {
                        match programs.get(program) {
                            None => diag.warning(
                                DiagCode::NotYetSimulated,
                                format!(
                                    "O{program} is not in this file (it may be stored on the \
                                     control); the call is skipped and later moves are drawn \
                                     without it"
                                ),
                                src,
                            ),
                            Some(_) if stack.len() >= MAX_CALL_DEPTH => diag.error(
                                DiagCode::NotYetSimulated,
                                format!(
                                    "subprogram calls nest more than {MAX_CALL_DEPTH} deep (does a \
                                     program call itself?); this call is skipped"
                                ),
                                src,
                            ),
                            Some(&header) => {
                                stack.push(Frame {
                                    return_to: pc + 1,
                                    start: header + 1,
                                    program: *program,
                                    remaining: *repeat - 1,
                                });
                                next = header + 1;
                            }
                        }
                    }
                    Command::ReturnFromSubprogram if stack.is_empty() => {
                        diag.info(
                            DiagCode::NotYetSimulated,
                            "M99 in the main program would run it again endlessly; it is drawn \
                             once and ends here",
                            src,
                        );
                        exec.ended = true;
                    }
                    Command::ReturnFromSubprogram => next = return_from(&mut stack),
                    _ => {}
                }
            }
        }
        pc = next;
    }

    // Anything never run: after the end, and not in a program that is
    // called. Program number blocks are not counted.
    let unrun = blocks.iter().enumerate().find(|(i, b)| {
        !ran.get(*i).copied().unwrap_or(true)
            && has_content(b)
            && dialect.program_number(b).is_none()
            && !in_a_called_subprogram(&programs, main_header, &ran, blocks, dialect, *i)
    });
    if let Some((index, block)) = unrun {
        diag.info(
            DiagCode::NotYetSimulated,
            "blocks from here are not run: they come after the end of the program and are not \
             part of a subprogram it calls",
            src_of(index, block),
        );
    }
    if exec.limit_reports > MAX_LIMIT_REPORTS {
        diag.info(
            DiagCode::OutOfLimits,
            format!(
                "{} more moves go outside the travel limits; they are flagged but not listed",
                exec.limit_reports - MAX_LIMIT_REPORTS
            ),
            SourceRef::default(),
        );
    }

    let meta = ProgramMeta {
        units_used: exec.units_used.clone(),
        tools_used: exec.tools_used.clone(),
        bbox: bounding_box(&exec.segments),
        estimated_time: Some(exec.segments.iter().filter_map(|s| s.time).sum()),
    };
    finish(exec.segments, diag, file_name, meta)
}

fn finish(segments: Vec<Segment>, diag: Diagnostics, file: &str, meta: ProgramMeta) -> Program {
    Program {
        channels: vec![ChannelStream {
            channel: 0,
            segments,
        }],
        diagnostics: dedupe(diag.into_vec()),
        files: vec![file.to_owned()],
        meta,
    }
}

/// Where to go at the end of a run of the current subprogram: its start
/// again if repeats remain, else the block after its call.
fn return_from(stack: &mut Vec<Frame>) -> usize {
    match stack.last_mut() {
        Some(frame) if frame.remaining > 0 => {
            frame.remaining -= 1;
            frame.start
        }
        Some(_) => stack.pop().map_or(usize::MAX, |f| f.return_to),
        None => usize::MAX,
    }
}

/// `true` if block `index` belongs to a subprogram that was run (some of
/// its blocks ran), so not running it was the program's choice (a branch
/// never reached), not code after the end. The main program is not a
/// subprogram: code after its end is reported.
fn in_a_called_subprogram(
    programs: &std::collections::BTreeMap<u32, usize>,
    main_header: Option<usize>,
    ran: &[bool],
    blocks: &[Block],
    dialect: &dyn Dialect,
    index: usize,
) -> bool {
    // The program a block belongs to starts at the last number block
    // before it.
    let Some(header) = (0..index).rev().find(|&i| {
        blocks
            .get(i)
            .is_some_and(|b| dialect.program_number(b).is_some())
    }) else {
        return false;
    };
    let is_program = Some(header) != main_header && programs.values().any(|&h| h == header);
    let end = (header + 1..blocks.len())
        .find(|&i| {
            blocks
                .get(i)
                .is_some_and(|b| dialect.program_number(b).is_some())
        })
        .unwrap_or(blocks.len());
    is_program
        && ran
            .get(header + 1..end)
            .is_some_and(|r| r.iter().any(|&x| x))
}

/// The diagnostics without exact repeats (same place, severity, code and
/// message), which a subprogram run several times would otherwise report
/// on every run. The first of each is kept, in order.
fn dedupe(diags: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut seen = BTreeSet::new();
    diags
        .into_iter()
        .filter(|d| {
            seen.insert((
                d.src.file,
                d.src.line,
                d.src.col_start,
                d.severity.id(),
                d.code as u16,
                d.message.clone(),
            ))
        })
        .collect()
}

/// `true` if the block would do something if executed.
fn has_content(block: &Block) -> bool {
    !block.words.is_empty() || !block.assignments.is_empty()
}

/// `block` without the M codes the machine file lists in `ignore_codes`, or
/// `None` if it has none of them. Each listed code is noted the first time
/// it appears.
fn strip_ignored(
    block: &Block,
    ignored: &BTreeSet<CodeKey>,
    noted: &mut BTreeSet<CodeKey>,
    src: SourceRef,
    diag: &mut Diagnostics,
) -> Option<Block> {
    let key = |w: &gcode_syntax::Word| {
        literal(&w.value)
            .and_then(|n| CodeKey::from_value(w.letter, n.value))
            .filter(|k| ignored.contains(k))
    };
    if !block.words.iter().any(|w| key(w).is_some()) {
        return None;
    }
    let mut kept = block.clone();
    kept.words.retain(|w| match key(w) {
        None => true,
        Some(k) => {
            if noted.insert(k) {
                // Only M codes can be listed (see `MachineDef::ignore_codes`).
                diag.info(
                    DiagCode::UnsupportedMCode,
                    format!(
                        "{k} is ignored, as the machine file lists it in ignore_codes. Not \
                         reported again."
                    ),
                    src,
                );
            }
            false
        }
    });
    Some(kept)
}

/// The dialect's view of the machine's axes.
fn axis_set(machine: &dyn Machine) -> Result<AxisSet, String> {
    let axes = machine
        .axes()
        .iter()
        .map(|a| AxisInfo {
            letter: a.name,
            arc_offset_letter: a.arc_offset_letter,
            rotary: a.kind == AxisKind::Rotary,
        })
        .collect();
    AxisSet::new(axes).map_err(|e| format!("machine axes are not usable: {e}"))
}

/// Machine state while running commands.
struct Executor<'m> {
    machine: &'m dyn Machine,
    state: ModalState,
    pos: Pos,
    work_offset: WorkOffsetId,
    origin: Pos,
    feed: f64,
    feed_mode: FeedMode,
    /// Whether a feed per revolution with the spindle stopped has been
    /// reported.
    stopped_feed_reported: bool,
    spindle_rpm: f64,
    spindle_on: bool,
    tool: u32,
    ended: bool,
    segments: Vec<Segment>,
    units_used: Vec<LengthUnit>,
    tools_used: Vec<u32>,
    checked_offsets: BTreeSet<WorkOffsetId>,
    limit_reports: usize,
    /// Whether each axis's position is known.
    known: [bool; MAX_AXES],
    /// Why some axis position became unknown.
    unknown_reason: String,
    /// Whether the current unknown-position episode has been reported.
    unknown_reported: bool,
}

impl<'m> Executor<'m> {
    fn new(state: ModalState, machine: &'m dyn Machine) -> Self {
        let work_offset = state.work_offset;
        let mut pos = [0.0; MAX_AXES];
        for (slot, axis) in pos.iter_mut().zip(machine.axes()) {
            *slot = axis.reference.unwrap_or(0.0);
        }
        Self {
            machine,
            pos,
            work_offset,
            origin: machine.work_offset(work_offset).unwrap_or([0.0; MAX_AXES]),
            feed: state.feed_rate.unwrap_or(0.0),
            feed_mode: state.feed_mode,
            stopped_feed_reported: false,
            spindle_rpm: state.spindle_rpm,
            spindle_on: state.spindle_state != SpindleState::Off,
            tool: state.current_tool.unwrap_or(0),
            units_used: vec![state.units],
            state,
            ended: false,
            segments: Vec::new(),
            tools_used: Vec::new(),
            checked_offsets: BTreeSet::new(),
            limit_reports: 0,
            known: [true; MAX_AXES],
            unknown_reason: String::new(),
            unknown_reported: false,
        }
    }

    fn apply(&mut self, command: &Command, src: SourceRef, diag: &mut Diagnostics) {
        match command {
            Command::Rapid { target } => {
                let end = self.resolve(target, src, diag);
                self.move_to(SegKind::Rapid, end, target, src, diag);
            }
            Command::Linear { target } => {
                let end = self.resolve(target, src, diag);
                self.move_to(SegKind::Linear, end, target, src, diag);
            }
            Command::Arc {
                target,
                center,
                dir,
                plane,
            } => {
                let end = self.resolve(target, src, diag);
                if self.start_known(src, diag) {
                    self.arc(end, center, *dir, *plane, src, diag);
                } else {
                    self.skip_to(end, target);
                }
            }
            Command::MachineRapid { target } => {
                let mut end = self.pos;
                for word in target {
                    let value = self.machine.normalize_axis_value(word.axis, word.value);
                    if let Some(slot) = end.get_mut(word.axis) {
                        *slot = value;
                    }
                }
                self.move_to(SegKind::Rapid, end, target, src, diag);
            }
            Command::ReturnToReference { via, position } => {
                self.return_to_reference(via, *position, src, diag);
            }
            Command::PositionUnknown { axes, reason } => {
                for &axis in axes {
                    if let Some(k) = self.known.get_mut(axis) {
                        *k = false;
                    }
                }
                self.unknown_reason.clone_from(reason);
                self.unknown_reported = false;
            }
            Command::Dwell { seconds } => {
                self.push(SegKind::Dwell, self.pos, None, src, diag);
                // A dwell of unknown length (Osai's DWT) comes as zero.
                if let Some(s) = self.segments.last_mut() {
                    s.time = (seconds.is_finite() && *seconds > 0.0).then_some(*seconds);
                }
            }
            Command::SetFeedRate(f) => self.feed = *f,
            Command::SetFeedMode(mode) => self.feed_mode = *mode,
            Command::SetUnits(u) => {
                if !self.units_used.contains(u) {
                    self.units_used.push(*u);
                }
            }
            Command::SetWorkOffset(id) => {
                self.work_offset = *id;
                self.origin = self.machine.work_offset(*id).unwrap_or([0.0; MAX_AXES]);
            }
            Command::SetSpindleSpeed { spindle: 0, rpm } => self.spindle_rpm = *rpm,
            Command::SetSpindleState { spindle: 0, state } => {
                self.spindle_on = *state != SpindleState::Off;
                let kind = match state {
                    SpindleState::Cw => SegKind::SpindleCw,
                    SpindleState::Ccw => SegKind::SpindleCcw,
                    SpindleState::Off => SegKind::SpindleStop,
                };
                self.push(kind, self.pos, None, src, diag);
            }
            Command::Coolant(state) => {
                let kind = match state {
                    CoolantState::Flood => SegKind::CoolantFlood,
                    CoolantState::Mist => SegKind::CoolantMist,
                    CoolantState::Off => SegKind::CoolantOff,
                };
                self.push(kind, self.pos, None, src, diag);
            }
            Command::Stop { optional } => {
                let kind = if *optional {
                    SegKind::OptionalStop
                } else {
                    SegKind::Stop
                };
                self.push(kind, self.pos, None, src, diag);
            }
            Command::ToolChange { tool } => {
                self.tool = *tool;
                if !self.tools_used.contains(tool) {
                    self.tools_used.push(*tool);
                }
                self.push(SegKind::ToolChange, self.pos, None, src, diag);
            }
            Command::ProgramEnd { .. } => {
                self.push(SegKind::ProgramEnd, self.pos, None, src, diag);
                self.ended = true;
            }
            // Modes the dialect has already applied to its state, spindles
            // other than the first (a single-spindle machine has no others),
            // and commands already reported by the dialect.
            Command::SetDistanceMode(_)
            | Command::SetPlane(_)
            | Command::SetSpindleSpeed { .. }
            | Command::SetSpindleState { .. }
            | Command::SelectTool(_)
            | Command::Unsupported { .. } => {}
            // Followed by the run loop, after the block's other commands.
            Command::CallSubprogram { .. } | Command::ReturnFromSubprogram => {}
        }
    }

    /// `true` if every axis's position is known, so a move can be drawn
    /// from here. Otherwise reports, once per episode, that moves are not
    /// shown.
    fn start_known(&mut self, src: SourceRef, diag: &mut Diagnostics) -> bool {
        let axes = self.machine.axes();
        let unknown: Vec<char> = axes
            .iter()
            .zip(&self.known)
            .filter(|(_, known)| !**known)
            .map(|(a, _)| a.name)
            .collect();
        if unknown.is_empty() {
            self.unknown_reported = false;
            return true;
        }
        if !self.unknown_reported {
            self.unknown_reported = true;
            let letters = unknown
                .iter()
                .map(char::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            diag.warning(
                DiagCode::NotYetSimulated,
                format!(
                    "the position of {letters} is unknown after {}, so moves are not shown \
                     until every axis has been given an absolute position",
                    self.unknown_reason
                ),
                src,
            );
        }
        false
    }

    /// Moves without drawing (the start is unknown). Axes given absolute
    /// values become known.
    fn skip_to(&mut self, end: Pos, words: &[AxisWord]) {
        self.mark_known(words);
        self.pos = end;
    }

    fn mark_known(&mut self, words: &[AxisWord]) {
        for word in words.iter().filter(|w| !w.incremental) {
            if let Some(k) = self.known.get_mut(word.axis) {
                *k = true;
            }
        }
    }

    /// A straight move: drawn if the start is known.
    fn move_to(
        &mut self,
        kind: SegKind,
        end: Pos,
        words: &[AxisWord],
        src: SourceRef,
        diag: &mut Diagnostics,
    ) {
        if self.start_known(src, diag) {
            self.push(kind, end, None, src, diag);
        } else {
            self.skip_to(end, words);
        }
    }

    /// Rapid to the intermediate point, then to the reference position on
    /// the same axes. Zero-length legs are not drawn.
    fn return_to_reference(
        &mut self,
        via: &[AxisWord],
        position: u8,
        src: SourceRef,
        diag: &mut Diagnostics,
    ) {
        let intermediate = self.resolve(via, src, diag);
        if intermediate != self.pos {
            self.move_to(SegKind::Rapid, intermediate, via, src, diag);
        }
        if position != 1 {
            // Only the first reference position is in the machine definition.
            for word in via {
                if let Some(k) = self.known.get_mut(word.axis) {
                    *k = false;
                }
            }
            self.unknown_reason = format!(
                "the return to reference position {position} on line {} (not in the machine \
                 file)",
                src.line
            );
            self.unknown_reported = false;
            return;
        }
        let mut end = self.pos;
        let mut reached = Vec::new();
        let mut missing = Vec::new();
        for word in via {
            let axis = self.machine.axes().get(word.axis);
            match (axis.and_then(|a| a.reference), end.get_mut(word.axis)) {
                (Some(reference), Some(slot)) => {
                    *slot = reference;
                    reached.push(AxisWord {
                        axis: word.axis,
                        value: reference,
                        incremental: false,
                    });
                }
                _ => missing.push((word.axis, axis.map_or('?', |a| a.name))),
            }
        }
        if end != self.pos {
            self.move_to(SegKind::Rapid, end, &reached, src, diag);
        } else {
            self.mark_known(&reached);
        }
        if !missing.is_empty() {
            let letters: String = missing.iter().map(|(_, l)| *l).collect();
            diag.error(
                DiagCode::InvalidValue,
                format!(
                    "the machine file gives no reference position for {letters}, so where the \
                     reference return leaves it is unknown"
                ),
                src,
            );
            for (axis, _) in missing {
                if let Some(k) = self.known.get_mut(axis) {
                    *k = false;
                }
            }
            self.unknown_reason = format!("the reference return on line {}", src.line);
            self.unknown_reported = false;
        }
    }

    /// End position of a move, in machine coordinates.
    fn resolve(&mut self, target: &[AxisWord], src: SourceRef, diag: &mut Diagnostics) -> Pos {
        // Report an undefined work offset once, at its first move.
        if !target.is_empty()
            && self.checked_offsets.insert(self.work_offset)
            && self.machine.work_offset(self.work_offset).is_none()
        {
            diag.error(
                DiagCode::InvalidValue,
                format!(
                    "work offset {} is not defined in the machine file; machine coordinates \
                     are used instead",
                    self.work_offset.0
                ),
                src,
            );
        }
        let mut end = self.pos;
        for word in target {
            let value = self.machine.normalize_axis_value(word.axis, word.value);
            let (Some(slot), Some(current), Some(origin)) = (
                end.get_mut(word.axis),
                self.pos.get(word.axis),
                self.origin.get(word.axis),
            ) else {
                continue;
            };
            *slot = if word.incremental {
                current + value
            } else {
                origin + value
            };
        }
        end
    }

    fn arc(
        &mut self,
        end: Pos,
        center: &ArcCenter,
        dir: ArcDir,
        plane: ArcPlane,
        src: SourceRef,
        diag: &mut Diagnostics,
    ) {
        let (ia, ib) = (usize::from(plane.first), usize::from(plane.second));
        // An end point that matches the start (within rounding error from
        // float arithmetic) is a full circle: make it match exactly, so the
        // IR's coincident-end-points rule applies.
        let mut end = end;
        if let (ArcCenter::Offset(_) | ArcCenter::Position(_), Some((sa, sb)), Some((ea, eb))) = (
            center,
            arc::plane_coords(&self.pos, plane),
            arc::plane_coords(&end, plane),
        ) {
            if (ea - sa).hypot(eb - sb) < SAME_POINT {
                if let Some(a) = end.get_mut(ia) {
                    *a = sa;
                }
                if let Some(b) = end.get_mut(ib) {
                    *b = sb;
                }
            }
        }
        let result = match center {
            ArcCenter::Offset(offsets) => {
                let mut c = self.pos;
                for off in offsets {
                    if let Some(slot) = c.get_mut(off.axis) {
                        *slot += off.value;
                    }
                }
                arc::plane_coords(&c, plane)
                    .ok_or(ArcProblem::BadPlane)
                    .and_then(|cc| Ok((cc, arc::from_center(&self.pos, &end, cc, plane)?)))
            }
            ArcCenter::Position(words) => {
                let mut c = self.pos;
                for word in words {
                    let value = self.machine.normalize_axis_value(word.axis, word.value);
                    let (Some(slot), Some(current), Some(origin)) = (
                        c.get_mut(word.axis),
                        self.pos.get(word.axis),
                        self.origin.get(word.axis),
                    ) else {
                        continue;
                    };
                    *slot = if word.incremental {
                        current + value
                    } else {
                        origin + value
                    };
                }
                arc::plane_coords(&c, plane)
                    .ok_or(ArcProblem::BadPlane)
                    .and_then(|cc| Ok((cc, arc::from_center(&self.pos, &end, cc, plane)?)))
            }
            ArcCenter::Radius(r) => arc::from_radius(&self.pos, &end, *r, dir == ArcDir::Cw, plane)
                .and_then(|cc| {
                    let (sa, sb) =
                        arc::plane_coords(&self.pos, plane).ok_or(ArcProblem::BadPlane)?;
                    Ok((cc, (sa - cc.0).hypot(sb - cc.1)))
                }),
        };
        let ((ca, cb), radius) = match result {
            Ok(v) => v,
            Err(problem) => {
                report_arc_problem(problem, src, diag);
                return;
            }
        };
        // Only the plane axes of the centre are meaningful.
        let mut center_pos = [0.0; MAX_AXES];
        if let Some(a) = center_pos.get_mut(ia) {
            *a = ca;
        }
        if let Some(b) = center_pos.get_mut(ib) {
            *b = cb;
        }
        let kind = match dir {
            ArcDir::Cw => SegKind::ArcCw,
            ArcDir::Ccw => SegKind::ArcCcw,
        };
        let geom = ArcGeom {
            center: center_pos,
            plane,
            radius,
            turns: 0,
        };
        self.push(kind, end, Some(geom), src, diag);
    }

    fn push(
        &mut self,
        kind: SegKind,
        end: Pos,
        arc: Option<ArcGeom>,
        src: SourceRef,
        diag: &mut Diagnostics,
    ) {
        let feed = match kind {
            SegKind::Linear | SegKind::ArcCw | SegKind::ArcCcw => self.feed_per_minute(src, diag),
            _ => 0.0,
        };
        let mut segment = Segment {
            kind,
            start: self.pos,
            end,
            arc,
            feed,
            spindle_rpm: if self.spindle_on {
                self.spindle_rpm
            } else {
                0.0
            },
            tool: self.tool,
            work_offset: self.work_offset,
            src,
            cycle_step: CycleStep::None,
            flags: SegFlags::EMPTY,
            time: None,
        };
        if kind.is_motion() {
            self.check_limits(&mut segment, diag);
        } else if self.known.iter().any(|k| !k) {
            // An event where the machine is not known to be: it is placed at
            // the last known position, so say so.
            segment.flags.insert(SegFlags::POSITION_UNKNOWN);
        }
        segment.time = self.estimate_time(&segment);
        self.pos = end;
        self.segments.push(segment);
    }

    /// The feed rate in mm/min for a feed move now: the programmed feed,
    /// or under feed per revolution the feed times the spindle speed. With
    /// the spindle stopped there is none (0), reported once.
    fn feed_per_minute(&mut self, src: SourceRef, diag: &mut Diagnostics) -> f64 {
        if self.feed_mode != FeedMode::PerRevolution {
            return self.feed;
        }
        let rpm = if self.spindle_on {
            self.spindle_rpm
        } else {
            0.0
        };
        if rpm > 0.0 && rpm.is_finite() {
            return self.feed * rpm;
        }
        if !self.stopped_feed_reported {
            self.stopped_feed_reported = true;
            diag.warning(
                DiagCode::MissingFeedRate,
                "feed per revolution with the spindle stopped: the move has no feed rate in \
                 mm/min, so it has no time in the estimate. Further cases are not reported.",
                src,
            );
        }
        0.0
    }

    /// The simple time estimate for `seg`, in seconds (no acceleration):
    ///
    /// - a rapid takes as long as its slowest axis at that axis's rapid
    ///   rate (unknown if a moving axis has none);
    /// - a feed move takes its length over the linear axes divided by the
    ///   feed rate, capped at the machine's maximum feed (unknown with no
    ///   feed rate, or when only rotary axes move);
    /// - a tool change takes the machine's tool change time (0 if not set);
    /// - other events take no time (a dwell's time is set by the caller).
    fn estimate_time(&self, seg: &Segment) -> Option<f64> {
        let axes = self.machine.axes();
        match seg.kind {
            SegKind::Rapid => {
                let mut time: f64 = 0.0;
                for (k, axis) in axes.iter().enumerate() {
                    let (s, e) = (seg.start.get(k)?, seg.end.get(k)?);
                    let distance = (e - s).abs();
                    if !distance.is_finite() {
                        return None;
                    }
                    if distance > 0.0 {
                        time = time.max(distance / axis.rapid_rate? * 60.0);
                    }
                }
                Some(time)
            }
            SegKind::Linear | SegKind::ArcCw | SegKind::ArcCcw => {
                let linear = |k: usize| axes.get(k).is_some_and(|a| a.kind == AxisKind::Linear);
                let length = path_length(seg, linear);
                let feed = match self.machine.speeds().max_feed {
                    Some(max) => seg.feed.min(max),
                    None => seg.feed,
                };
                if !(length.is_finite() && feed > 0.0 && feed.is_finite()) {
                    return None;
                }
                let rotary_moves = seg
                    .start
                    .iter()
                    .zip(&seg.end)
                    .enumerate()
                    .any(|(k, (s, e))| s != e && !linear(k));
                if length == 0.0 && rotary_moves {
                    return None;
                }
                Some(length / feed * 60.0)
            }
            SegKind::ToolChange => Some(self.machine.speeds().tool_change_time.unwrap_or(0.0)),
            _ => Some(0.0),
        }
    }

    /// Flags (and reports) a segment that goes outside the travel limits
    /// anywhere along it, including arc bulges.
    fn check_limits(&mut self, segment: &mut Segment, diag: &mut Diagnostics) {
        let Some(bbox) = bounding_box(std::slice::from_ref(&*segment)) else {
            return;
        };
        // Both corners can be past a limit on the same axis: keep, per
        // axis, the point furthest outside its travel.
        let mut violations: Vec<LimitViolation> = Vec::new();
        for v in self
            .machine
            .check_limits(&bbox.min)
            .into_iter()
            .chain(self.machine.check_limits(&bbox.max))
        {
            let outside = |v: &LimitViolation| (v.min - v.value).max(v.value - v.max);
            match violations.iter_mut().find(|w| w.axis == v.axis) {
                Some(w) if outside(&v) > outside(w) => *w = v,
                Some(_) => {}
                None => violations.push(v),
            }
        }
        if violations.is_empty() {
            return;
        }
        segment.flags.insert(SegFlags::OUT_OF_LIMITS);
        self.limit_reports += 1;
        if self.limit_reports > MAX_LIMIT_REPORTS {
            return;
        }
        let axes = self.machine.axes();
        let details: Vec<String> = violations
            .iter()
            .map(|v| {
                let name = axes.get(v.axis).map_or('?', |a| a.name);
                format!(
                    "{name} reaches {:.3} (travel {:.3} to {:.3})",
                    v.value, v.min, v.max
                )
            })
            .collect();
        diag.push(Diagnostic::warning(
            DiagCode::OutOfLimits,
            format!(
                "move goes outside the travel limits: {}",
                details.join(", ")
            ),
            segment.src,
        ));
    }
}

fn report_arc_problem(problem: ArcProblem, src: SourceRef, diag: &mut Diagnostics) {
    let (code, message) = match problem {
        ArcProblem::BadPlane => (
            DiagCode::InvalidArc,
            "arc plane does not match the machine's axes; block not moved".to_owned(),
        ),
        ArcProblem::RadiusMismatch { start, end } => (
            DiagCode::ArcRadiusMismatch,
            format!(
                "arc start is {start:.4} from the centre but the end is {end:.4} (more than \
                 {RADIUS_TOLERANCE} apart); block not moved"
            ),
        ),
        ArcProblem::RadiusTooSmall { radius, half_chord } => (
            DiagCode::InvalidArc,
            format!(
                "radius {radius:.4} is too small to reach the end point (needs at least \
                 {half_chord:.4}); block not moved"
            ),
        ),
        ArcProblem::ZeroLength => (
            DiagCode::InvalidArc,
            "arc with R ends where it starts, so it has no defined size; block not moved"
                .to_owned(),
        ),
    };
    diag.error(code, message, src);
}
