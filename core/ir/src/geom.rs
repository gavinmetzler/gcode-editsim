//! Positions, arc geometry and bounding boxes.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::segment::{SegKind, Segment};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Maximum number of axes a position can hold.
pub const MAX_AXES: usize = 9;

/// A machine position, indexed by the machine's axis list.
///
/// Index `i` is the machine's `i`-th axis; unused trailing entries are `0.0`
/// and ignored. Linear axes are in mm, rotary axes in degrees.
pub type Pos = [f64; MAX_AXES];

/// The plane an arc lies in, as two indices into the machine's axis list.
///
/// The order matters: a counter-clockwise arc turns from the `first` axis
/// toward the `second` axis (the usual right-hand convention, viewed from
/// the positive end of the third axis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ArcPlane {
    /// Index of the plane's first axis.
    pub first: u8,
    /// Index of the plane's second axis.
    pub second: u8,
}

impl ArcPlane {
    /// Both axis indices as `usize`, or `None` if either is out of range or
    /// they are the same axis.
    pub fn indices(self) -> Option<(usize, usize)> {
        let (a, b) = (usize::from(self.first), usize::from(self.second));
        (a < MAX_AXES && b < MAX_AXES && a != b).then_some((a, b))
    }
}

/// Geometry of a circular or helical arc.
///
/// Together with its segment's `start`, `end` and `kind` this fully
/// describes the arc:
///
/// - It turns around `center` in `plane`, clockwise or counter-clockwise
///   according to the segment kind.
/// - The angle swept is the shortest turn in that direction from `start`
///   to `end`, plus `turns` full revolutions. If `start` and `end` are at the
///   same angle the arc is a full circle (not zero length).
/// - Axes outside the plane move linearly from `start` to `end` over the
///   arc, which makes a helix when one of them changes.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ArcGeom {
    /// Absolute centre. Only the two plane axes are meaningful.
    pub center: Pos,
    /// Plane of rotation.
    pub plane: ArcPlane,
    /// Radius in mm.
    pub radius: f64,
    /// Extra full revolutions beyond the basic sweep; usually 0.
    pub turns: u16,
}

/// Axis-aligned bounds over every axis of [`Pos`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BoundingBox {
    /// Smallest value reached on each axis.
    pub min: Pos,
    /// Largest value reached on each axis.
    pub max: Pos,
}

impl BoundingBox {
    /// A box containing just `p`.
    pub fn from_point(p: &Pos) -> Self {
        Self { min: *p, max: *p }
    }

    /// Grows the box to contain `p`.
    pub fn include(&mut self, p: &Pos) {
        for ((lo, hi), v) in self.min.iter_mut().zip(self.max.iter_mut()).zip(p) {
            *lo = lo.min(*v);
            *hi = hi.max(*v);
        }
    }

    /// Grows the box so that axis `axis` covers `value`. Out-of-range axes
    /// are ignored.
    fn include_axis(&mut self, axis: usize, value: f64) {
        if let (Some(lo), Some(hi)) = (self.min.get_mut(axis), self.max.get_mut(axis)) {
            *lo = lo.min(value);
            *hi = hi.max(value);
        }
    }
}

/// Computes the bounding box of the motion segments in `segments`.
///
/// Only motion kinds (rapid, linear, arcs) are counted; dwells, tool changes
/// and sync points are skipped. Filter the input first to bound, say, only
/// cutting moves. Arcs are bounded exactly: the furthest points of the
/// circle are included wherever the arc actually passes through them, not
/// just its end points.
///
/// Returns `None` if there are no motion segments.
pub fn bounding_box<'a, I>(segments: I) -> Option<BoundingBox>
where
    I: IntoIterator<Item = &'a Segment>,
{
    let mut bbox: Option<BoundingBox> = None;
    for seg in segments.into_iter().filter(|s| s.kind.is_motion()) {
        let b = bbox.get_or_insert_with(|| BoundingBox::from_point(&seg.start));
        b.include(&seg.start);
        b.include(&seg.end);
        if let (true, Some(arc)) = (seg.kind.is_arc(), &seg.arc) {
            include_arc_extremes(b, seg, arc);
        }
    }
    bbox
}

/// Adds the points where the arc reaches its furthest extent along the two
/// plane axes (angles 0°, 90°, 180°, 270° around the centre), for each one
/// the arc actually sweeps through.
fn include_arc_extremes(b: &mut BoundingBox, seg: &Segment, arc: &ArcGeom) {
    let Some((ia, ib)) = arc.plane.indices() else {
        return;
    };
    let r = arc.radius;
    if !(r.is_finite() && r > 0.0) {
        return;
    }
    let (Some(&ca), Some(&cb)) = (arc.center.get(ia), arc.center.get(ib)) else {
        return;
    };
    let angle_of = |p: &Pos| -> Option<f64> {
        let (a, b) = (p.get(ia)?, p.get(ib)?);
        Some((b - cb).atan2(a - ca))
    };
    let (Some(start), Some(end)) = (angle_of(&seg.start), angle_of(&seg.end)) else {
        return;
    };
    let ccw = seg.kind == SegKind::ArcCcw;

    // Angle swept from start to end in the direction of travel.
    let mut sweep = if ccw {
        (end - start).rem_euclid(TAU)
    } else {
        (start - end).rem_euclid(TAU)
    };
    if sweep < ANGLE_EPS {
        sweep = TAU; // coincident end points: full circle
    }
    sweep += f64::from(arc.turns) * TAU;

    let extremes = [
        (0.0, (ca + r, cb)),
        (FRAC_PI_2, (ca, cb + r)),
        (PI, (ca - r, cb)),
        (PI + FRAC_PI_2, (ca, cb - r)),
    ];
    for (angle, (a, bv)) in extremes {
        let offset = if ccw {
            (angle - start).rem_euclid(TAU)
        } else {
            (start - angle).rem_euclid(TAU)
        };
        if sweep >= TAU || offset <= sweep + ANGLE_EPS {
            b.include_axis(ia, a);
            b.include_axis(ib, bv);
        }
    }
}

/// Angular tolerance (radians) for "same angle" comparisons.
const ANGLE_EPS: f64 = 1e-12;
