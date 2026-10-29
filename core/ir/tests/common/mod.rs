//! Helpers shared by the integration tests.

#![allow(dead_code)]

use ir::{ArcGeom, ArcPlane, CycleStep, Pos, SegFlags, SegKind, Segment, SourceRef, WorkOffsetId};

/// A position with the given leading axis values and zeros elsewhere.
pub fn pos(values: &[f64]) -> Pos {
    let mut p = [0.0; ir::MAX_AXES];
    for (slot, v) in p.iter_mut().zip(values) {
        *slot = *v;
    }
    p
}

/// A segment of `kind` from `start` to `end` on source `line`.
pub fn seg(kind: SegKind, start: Pos, end: Pos, line: u32) -> Segment {
    Segment {
        kind,
        start,
        end,
        arc: None,
        feed: 0.0,
        spindle_rpm: 0.0,
        tool: 0,
        work_offset: WorkOffsetId::default(),
        src: SourceRef {
            line,
            ..SourceRef::default()
        },
        cycle_step: CycleStep::None,
        flags: SegFlags::EMPTY,
    }
}

/// A linear segment between two points on line 1.
pub fn line_seg(start: &[f64], end: &[f64]) -> Segment {
    seg(SegKind::Linear, pos(start), pos(end), 1)
}

/// An arc in the plane of axes 0 and 1 around `center`.
pub fn arc(kind: SegKind, start: &[f64], end: &[f64], center: &[f64], radius: f64) -> Segment {
    arc_in(
        kind,
        start,
        end,
        center,
        radius,
        ArcPlane {
            first: 0,
            second: 1,
        },
    )
}

/// An arc in an explicit plane.
pub fn arc_in(
    kind: SegKind,
    start: &[f64],
    end: &[f64],
    center: &[f64],
    radius: f64,
    plane: ArcPlane,
) -> Segment {
    let mut s = seg(kind, pos(start), pos(end), 1);
    s.arc = Some(ArcGeom {
        center: pos(center),
        plane,
        radius,
        turns: 0,
    });
    s
}

/// A segment from each line number in `lines`, in order.
pub fn segs_on_lines(lines: &[u32]) -> Vec<Segment> {
    lines
        .iter()
        .map(|&l| seg(SegKind::Linear, pos(&[]), pos(&[]), l))
        .collect()
}
