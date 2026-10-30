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

/// An arc's angles around its centre, in its plane.
struct Sweep {
    /// Plane axis indices.
    ia: usize,
    ib: usize,
    /// Centre in plane coordinates.
    ca: f64,
    cb: f64,
    /// Angle of the start point (radians, from the first plane axis).
    start: f64,
    /// Angle swept in the direction of travel (radians, > 0).
    sweep: f64,
    /// Counter-clockwise (angles increase) or clockwise.
    ccw: bool,
}

impl Sweep {
    /// Works out the sweep of `seg`'s arc, or `None` if the plane or centre
    /// is unusable.
    fn of(seg: &Segment, arc: &ArcGeom) -> Option<Sweep> {
        let (ia, ib) = arc.plane.indices()?;
        let (&ca, &cb) = (arc.center.get(ia)?, arc.center.get(ib)?);
        let angle_of = |p: &Pos| -> Option<f64> {
            let (a, b) = (p.get(ia)?, p.get(ib)?);
            Some((b - cb).atan2(a - ca))
        };
        let (start, end) = (angle_of(&seg.start)?, angle_of(&seg.end)?);
        let ccw = seg.kind == SegKind::ArcCcw;
        let mut sweep = if ccw {
            (end - start).rem_euclid(TAU)
        } else {
            (start - end).rem_euclid(TAU)
        };
        if sweep < ANGLE_EPS {
            sweep = TAU; // coincident end points: full circle
        }
        sweep += f64::from(arc.turns) * TAU;
        sweep.is_finite().then_some(Sweep {
            ia,
            ib,
            ca,
            cb,
            start,
            sweep,
            ccw,
        })
    }

    /// Distance of `p` from the centre in the plane.
    fn radius_of(&self, p: &Pos) -> f64 {
        let a = p.get(self.ia).copied().unwrap_or(0.0);
        let b = p.get(self.ib).copied().unwrap_or(0.0);
        (a - self.ca).hypot(b - self.cb)
    }
}

/// Adds the points where the arc reaches its furthest extent along the two
/// plane axes (angles 0°, 90°, 180°, 270° around the centre), for each one
/// the arc actually sweeps through.
fn include_arc_extremes(b: &mut BoundingBox, seg: &Segment, arc: &ArcGeom) {
    let r = arc.radius;
    if !(r.is_finite() && r > 0.0) {
        return;
    }
    let Some(Sweep {
        ia,
        ib,
        ca,
        cb,
        start,
        sweep,
        ccw,
    }) = Sweep::of(seg, arc)
    else {
        return;
    };

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

/// Most points [`tessellate`] produces for one arc. A smaller tolerance
/// (or many `turns`) is not met beyond this, rather than using unbounded
/// memory.
pub const MAX_ARC_POINTS: usize = 100_000;

/// Largest angle (radians) between two tessellated points, however coarse
/// the tolerance, so a circle never collapses to fewer than four chords.
const MAX_STEP: f64 = FRAC_PI_2;

/// The points along a segment, for drawing or simulation.
///
/// - **Motion** gives its start and end; an **arc** also gives points in
///   between, close enough that no chord between neighbouring points is
///   further than `tol` (mm) from the true arc. A coarser tolerance still
///   gives at least one point every 90°.
/// - **Events** (dwell, tool change, stops, …) give their single position.
///
/// The first point is exactly `start` and the last exactly `end`. Arc
/// points lie on the arc; axes outside the plane move linearly, which
/// follows a helix exactly. If start and end are at slightly different
/// distances from the centre (within the interpreter's tolerance), the
/// radius changes evenly along the arc so the ends still match.
///
/// An arc whose plane or centre is unusable is drawn as a straight line.
/// A tolerance that is not a positive number, or needs more than
/// [`MAX_ARC_POINTS`] points, gives [`MAX_ARC_POINTS`] points.
pub fn tessellate(seg: &Segment, tol: f64) -> impl Iterator<Item = Pos> + '_ {
    let (steps, sweep) = if !seg.kind.is_motion() {
        (0, None)
    } else {
        match seg.arc.as_ref().filter(|_| seg.kind.is_arc()) {
            Some(arc) => match Sweep::of(seg, arc) {
                Some(sw) => {
                    let r = sw.radius_of(&seg.start).max(sw.radius_of(&seg.end));
                    (arc_steps(r, sw.sweep, tol), Some(sw))
                }
                None => (1, None),
            },
            None => (1, None),
        }
    };
    (0..=steps).map(move |i| point_at(seg, sweep.as_ref(), i, steps))
}

/// Number of chords for an arc of radius `r` sweeping `sweep` radians.
fn arc_steps(r: f64, sweep: f64, tol: f64) -> usize {
    // `tol > 0.0` is false for NaN; an infinite tolerance is just coarse.
    let max_step = if !(tol > 0.0 && r.is_finite()) {
        return MAX_ARC_POINTS - 1;
    } else if tol >= r {
        MAX_STEP
    } else {
        // Chord error (sagitta) of a step θ is r·(1 − cos(θ/2)).
        (2.0 * (1.0 - tol / r).acos()).min(MAX_STEP)
    };
    let steps = (sweep / max_step).ceil();
    if steps.is_finite() && steps >= 1.0 {
        // Saturating float-to-int conversion, then capped.
        (steps as usize).min(MAX_ARC_POINTS - 1)
    } else if steps >= 1.0 {
        MAX_ARC_POINTS - 1
    } else {
        1
    }
}

/// Point `i` of `steps` along `seg`.
fn point_at(seg: &Segment, sweep: Option<&Sweep>, i: usize, steps: usize) -> Pos {
    if i == 0 {
        return seg.start;
    }
    if i >= steps {
        return seg.end;
    }
    let t = i as f64 / steps as f64;
    let mut p = seg.start;
    for ((v, s), e) in p.iter_mut().zip(&seg.start).zip(&seg.end) {
        *v = s + (e - s) * t;
    }
    if let Some(sw) = sweep {
        let (rs, re) = (sw.radius_of(&seg.start), sw.radius_of(&seg.end));
        let r = rs + (re - rs) * t;
        let turned = sw.sweep * t;
        let angle = if sw.ccw {
            sw.start + turned
        } else {
            sw.start - turned
        };
        if let Some(a) = p.get_mut(sw.ia) {
            *a = sw.ca + r * angle.cos();
        }
        if let Some(b) = p.get_mut(sw.ib) {
            *b = sw.cb + r * angle.sin();
        }
    }
    p
}
