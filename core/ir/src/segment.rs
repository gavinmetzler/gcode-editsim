//! Segments, channels and the program container.

use std::ops::Range;

use crate::diag::Diagnostic;
use crate::geom::{ArcGeom, BoundingBox, Pos};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Where in the source text something came from.
///
/// Every segment and diagnostic carries one so the editor can map between
/// text and toolpath in both directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SourceRef {
    /// Index into [`Program::files`].
    pub file: u16,
    /// 1-based line number.
    pub line: u32,
    /// 0-based block index within the parsed program.
    pub block: u32,
    /// Start column of the relevant span (0 if unknown).
    pub col_start: u16,
    /// End column of the relevant span (0 if unknown).
    pub col_end: u16,
}

/// What kind of segment this is.
///
/// Motion kinds move the tool from `start` to `end`. Every other kind is a
/// zero-length *event* at the position where it happened (`start == end`),
/// kept in the segment list so that everything stays in program order.
///
/// The numeric values are part of the render-buffer format and must not
/// change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum SegKind {
    /// Positioning move at rapid rate.
    Rapid = 0,
    /// Straight move at feed rate.
    Linear = 1,
    /// Clockwise arc (see [`ArcGeom`] for orientation).
    ArcCw = 2,
    /// Counter-clockwise arc (see [`ArcGeom`] for orientation).
    ArcCcw = 3,
    /// Pause with no motion.
    Dwell = 4,
    /// Tool change with no programmed motion.
    ToolChange = 5,
    /// Synchronisation point between channels.
    Sync = 6,
    /// Program stop: execution pauses until the operator restarts it.
    Stop = 7,
    /// Optional stop: pauses only if the operator's optional-stop switch
    /// is on.
    OptionalStop = 8,
    /// Spindle started clockwise.
    SpindleCw = 9,
    /// Spindle started counter-clockwise.
    SpindleCcw = 10,
    /// Spindle stopped.
    SpindleStop = 11,
    /// Flood coolant on.
    CoolantFlood = 12,
    /// Mist coolant on.
    CoolantMist = 13,
    /// All coolant off.
    CoolantOff = 14,
    /// End of the program. Nothing after it is executed.
    ProgramEnd = 15,
}

impl SegKind {
    /// Every kind, in value order (index = value). Used to generate the
    /// TypeScript constants (`tools/gen-ts`).
    pub const ALL: [SegKind; 16] = [
        SegKind::Rapid,
        SegKind::Linear,
        SegKind::ArcCw,
        SegKind::ArcCcw,
        SegKind::Dwell,
        SegKind::ToolChange,
        SegKind::Sync,
        SegKind::Stop,
        SegKind::OptionalStop,
        SegKind::SpindleCw,
        SegKind::SpindleCcw,
        SegKind::SpindleStop,
        SegKind::CoolantFlood,
        SegKind::CoolantMist,
        SegKind::CoolantOff,
        SegKind::ProgramEnd,
    ];

    /// `true` for kinds that move the tool from `start` to `end`.
    pub fn is_motion(self) -> bool {
        matches!(
            self,
            SegKind::Rapid | SegKind::Linear | SegKind::ArcCw | SegKind::ArcCcw
        )
    }

    /// `true` for zero-length kinds: everything that is not motion.
    pub fn is_event(self) -> bool {
        !self.is_motion()
    }

    /// `true` for [`SegKind::ArcCw`] and [`SegKind::ArcCcw`].
    pub fn is_arc(self) -> bool {
        matches!(self, SegKind::ArcCw | SegKind::ArcCcw)
    }
}

/// Which step of an expanded canned cycle a segment belongs to.
///
/// The numeric values are reserved for the render-buffer format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
#[non_exhaustive]
pub enum CycleStep {
    /// Not part of a canned cycle.
    #[default]
    None = 0,
    /// Rapid move to the cycle's start or clearance level.
    Approach = 1,
    /// Feed move into the material.
    FeedIn = 2,
    /// Dwell at the bottom of a step.
    Dwell = 3,
    /// Short retract between pecks to break or clear chips.
    PeckClear = 4,
    /// Retract out of the hole at the end of the cycle.
    Retract = 5,
}

/// Bit flags describing a segment's state.
///
/// Only the low byte is stored in the render buffer, so the most commonly
/// displayed flags use bits 0–7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct SegFlags(pub u32);

impl SegFlags {
    /// No flags set.
    pub const EMPTY: SegFlags = SegFlags(0);
    /// Cutter radius compensation was active for this segment.
    pub const COMP_ACTIVE: SegFlags = SegFlags(1 << 0);
    /// Some part of the segment lies outside the machine's travel limits.
    pub const OUT_OF_LIMITS: SegFlags = SegFlags(1 << 1);
    /// An event segment that happened while the machine position was not
    /// known, so its `start`/`end` is only the last known position. Never
    /// set on motion, which is not drawn from an unknown position.
    pub const POSITION_UNKNOWN: SegFlags = SegFlags(1 << 2);

    /// Every named flag with its name. Used to generate the TypeScript
    /// constants (`tools/gen-ts`).
    pub const NAMED: [(&'static str, SegFlags); 3] = [
        ("COMP_ACTIVE", SegFlags::COMP_ACTIVE),
        ("OUT_OF_LIMITS", SegFlags::OUT_OF_LIMITS),
        ("POSITION_UNKNOWN", SegFlags::POSITION_UNKNOWN),
    ];

    /// `true` if every flag in `other` is set in `self`.
    pub fn contains(self, other: SegFlags) -> bool {
        self.0 & other.0 == other.0
    }

    /// Sets every flag in `other`.
    pub fn insert(&mut self, other: SegFlags) {
        self.0 |= other.0;
    }

    /// Clears every flag in `other`.
    pub fn remove(&mut self, other: SegFlags) {
        self.0 &= !other.0;
    }

    /// `true` if no flags are set.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for SegFlags {
    type Output = SegFlags;

    fn bitor(self, rhs: SegFlags) -> SegFlags {
        SegFlags(self.0 | rhs.0)
    }
}

/// Identifies the work coordinate system active for a segment.
///
/// The value is assigned by the dialect and is opaque to everything
/// downstream of it; `0` means machine coordinates (no work offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct WorkOffsetId(pub u16);

/// One primitive piece of the toolpath.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Segment {
    /// What kind of segment this is.
    pub kind: SegKind,
    /// Machine position at the start, in internal units (mm, degrees).
    pub start: Pos,
    /// Machine position at the end. Equal to `start` for non-motion kinds.
    pub end: Pos,
    /// Arc geometry. Present for [`SegKind::ArcCw`] and [`SegKind::ArcCcw`]
    /// only.
    pub arc: Option<ArcGeom>,
    /// Feed rate in mm/min; `0.0` for rapids and dwells.
    pub feed: f64,
    /// Spindle speed in rpm.
    pub spindle_rpm: f64,
    /// Active tool number.
    pub tool: u32,
    /// Active work coordinate system.
    pub work_offset: WorkOffsetId,
    /// Source location of the block that produced this segment.
    pub src: SourceRef,
    /// Canned-cycle step, or [`CycleStep::None`].
    pub cycle_step: CycleStep,
    /// State flags.
    pub flags: SegFlags,
}

/// The segments of one independent execution stream.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChannelStream {
    /// Channel number.
    pub channel: u8,
    /// Segments in program order.
    pub segments: Vec<Segment>,
}

impl ChannelStream {
    /// The segments that came from `line`; see [`segments_for_line`].
    pub fn segments_for_line(&self, line: u32) -> Range<usize> {
        segments_for_line(&self.segments, line)
    }
}

/// Length unit a program was written in.
///
/// Everything in the IR is already converted to internal units (mm); this
/// only records what the source used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LengthUnit {
    /// Metric (mm).
    Metric,
    /// Imperial (inches).
    Inch,
}

/// Summary information about a program.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProgramMeta {
    /// Length units the program selected, in order of first use.
    pub units_used: Vec<LengthUnit>,
    /// Tool numbers used, in order of first use.
    pub tools_used: Vec<u32>,
    /// Bounding box of all motion, or `None` if there was no motion.
    pub bbox: Option<BoundingBox>,
    /// Estimated run time in seconds, if it has been computed.
    pub estimated_time: Option<f64>,
}

/// The complete output of interpreting a program.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Program {
    /// One stream per channel; a single-channel machine has exactly one.
    pub channels: Vec<ChannelStream>,
    /// Every problem found, from all stages.
    pub diagnostics: Vec<Diagnostic>,
    /// File table that [`SourceRef::file`] indexes into.
    pub files: Vec<String>,
    /// Summary information.
    pub meta: ProgramMeta,
}

/// Returns the index range of the segments that came from source `line`.
///
/// Uses binary search, so `segments` must be in non-decreasing
/// `src.line` order. That holds for straight-line programs in a single file,
/// but not for programs with subprograms (M3.7), whose lines run several
/// times; for those, build a line index (the UI does: `buildLineIndex` in
/// `ui/src/editor/lines.ts`). If the line produced no
/// segments the range is empty and starts where such segments would be
/// inserted, i.e. at the first segment from a later line.
///
/// If the order requirement is broken the result is unspecified but the
/// function still does not panic.
pub fn segments_for_line(segments: &[Segment], line: u32) -> Range<usize> {
    let start = segments.partition_point(|s| s.src.line < line);
    let end = segments.partition_point(|s| s.src.line <= line);
    // With unsorted input `end` can come before `start`; keep the range valid.
    start..end.max(start)
}
