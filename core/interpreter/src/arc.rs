//! Arc centre and radius from programmed values.

use ir::{ArcPlane, Pos};

/// Arc end points may differ in distance from the centre by this much (mm)
/// before the arc is rejected. Real controls make this a parameter.
pub(crate) const RADIUS_TOLERANCE: f64 = 0.01;

/// Points closer than this (mm) are the same point. Far below any
/// programmable increment (0.001 mm, 0.0001 inch), far above floating-point
/// drift from long chains of incremental moves (~1e-10 mm).
pub(crate) const SAME_POINT: f64 = 1e-6;

/// The two plane coordinates of `p`.
pub(crate) fn plane_coords(p: &Pos, plane: ArcPlane) -> Option<(f64, f64)> {
    Some((
        *p.get(usize::from(plane.first))?,
        *p.get(usize::from(plane.second))?,
    ))
}

/// Why an arc cannot be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ArcProblem {
    /// The plane indices are outside the position.
    BadPlane,
    /// Start and end are at different distances from the centre.
    RadiusMismatch { start: f64, end: f64 },
    /// The radius is shorter than half the distance between the end points.
    RadiusTooSmall { radius: f64, half_chord: f64 },
    /// R given with start and end at the same point.
    ZeroLength,
}

/// Centre (in plane coordinates) and radius from offsets already added to
/// the start point.
pub(crate) fn from_center(
    start: &Pos,
    end: &Pos,
    center: (f64, f64),
    plane: ArcPlane,
) -> Result<f64, ArcProblem> {
    let (sa, sb) = plane_coords(start, plane).ok_or(ArcProblem::BadPlane)?;
    let (ea, eb) = plane_coords(end, plane).ok_or(ArcProblem::BadPlane)?;
    let r_start = (sa - center.0).hypot(sb - center.1);
    let r_end = (ea - center.0).hypot(eb - center.1);
    if (r_start - r_end).abs() > RADIUS_TOLERANCE {
        return Err(ArcProblem::RadiusMismatch {
            start: r_start,
            end: r_end,
        });
    }
    Ok(r_start)
}

/// Centre (in plane coordinates) from a signed radius: positive for the
/// arc of 180° or less, negative for the longer arc.
pub(crate) fn from_radius(
    start: &Pos,
    end: &Pos,
    radius: f64,
    clockwise: bool,
    plane: ArcPlane,
) -> Result<(f64, f64), ArcProblem> {
    let (sa, sb) = plane_coords(start, plane).ok_or(ArcProblem::BadPlane)?;
    let (ea, eb) = plane_coords(end, plane).ok_or(ArcProblem::BadPlane)?;
    let (ua, ub) = (ea - sa, eb - sb);
    let chord = ua.hypot(ub);
    if chord < SAME_POINT {
        return Err(ArcProblem::ZeroLength);
    }
    let r = radius.abs();
    let half = chord / 2.0;
    let h = if r + RADIUS_TOLERANCE < half {
        return Err(ArcProblem::RadiusTooSmall {
            radius: r,
            half_chord: half,
        });
    } else if r <= half {
        0.0
    } else {
        (r * r - half * half).sqrt()
    };
    // Unit normal pointing left of the direction of travel.
    let (na, nb) = (-ub / chord, ua / chord);
    // Counter-clockwise short arcs have their centre on the left; clockwise
    // or long (negative R) arcs flip it, and both together flip it back.
    let left = clockwise == (radius < 0.0);
    let side = if left { h } else { -h };
    Ok((sa + ua / 2.0 + side * na, sb + ub / 2.0 + side * nb))
}
