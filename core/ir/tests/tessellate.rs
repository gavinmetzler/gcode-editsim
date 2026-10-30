//! Tests for `tessellate`: the points along a segment, especially arcs.

mod common;

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use common::{arc, arc_in, line_seg, pos, seg};
use ir::{bounding_box, tessellate, ArcPlane, Pos, SegKind, Segment, MAX_ARC_POINTS};
use proptest::prelude::*;

const EPS: f64 = 1e-9;

fn points(s: &Segment, tol: f64) -> Vec<Pos> {
    tessellate(s, tol).collect()
}

/// Distance from `c` in the plane of axes `a`, `b`.
fn dist(p: &Pos, c: &Pos, a: usize, b: usize) -> f64 {
    (p[a] - c[a]).hypot(p[b] - c[b])
}

fn quarter_ccw() -> Segment {
    // Centre (0, 0), radius 10, from 0° to 90°.
    arc(
        SegKind::ArcCcw,
        &[10.0, 0.0],
        &[0.0, 10.0],
        &[0.0, 0.0],
        10.0,
    )
}

#[test]
fn straight_moves_give_their_end_points() {
    for kind in [SegKind::Rapid, SegKind::Linear] {
        let s = seg(kind, pos(&[1.0, 2.0, 3.0]), pos(&[4.0, 5.0, 6.0]), 1);
        assert_eq!(points(&s, 0.01), vec![s.start, s.end]);
    }
    // Zero-length motion still gives two points.
    let s = line_seg(&[1.0], &[1.0]);
    assert_eq!(points(&s, 0.01).len(), 2);
}

#[test]
fn events_give_one_point() {
    let p = pos(&[7.0, 8.0, 9.0]);
    for kind in [
        SegKind::Dwell,
        SegKind::ToolChange,
        SegKind::Stop,
        SegKind::SpindleCw,
        SegKind::CoolantOff,
        SegKind::ProgramEnd,
    ] {
        assert_eq!(points(&seg(kind, p, p, 1), 0.01), vec![p], "{kind:?}");
    }
}

#[test]
fn arc_ends_are_exact_and_points_lie_on_the_arc() {
    let s = quarter_ccw();
    let pts = points(&s, 0.001);
    assert_eq!(pts.first(), Some(&s.start));
    assert_eq!(pts.last(), Some(&s.end));
    assert!(pts.len() > 3);
    let c = pos(&[0.0, 0.0]);
    for p in &pts {
        assert!((dist(p, &c, 0, 1) - 10.0).abs() < EPS, "{p:?}");
    }
}

#[test]
fn ccw_turns_one_way_and_cw_the_other() {
    let ccw = points(&quarter_ccw(), 0.01);
    // Every point of the CCW quarter is in the first quadrant.
    assert!(ccw.iter().all(|p| p[0] >= -EPS && p[1] >= -EPS));

    // Same end points clockwise is the long way round (270°) through the
    // other three quadrants.
    let cw = arc(
        SegKind::ArcCw,
        &[10.0, 0.0],
        &[0.0, 10.0],
        &[0.0, 0.0],
        10.0,
    );
    let pts = points(&cw, 0.01);
    assert!(pts.iter().any(|p| p[0] < -9.9));
    assert!(pts.iter().any(|p| p[1] < -9.9));
}

#[test]
fn chord_error_within_tolerance() {
    let s = quarter_ccw();
    for tol in [1.0, 0.1, 0.01, 0.001] {
        let pts = points(&s, tol);
        for w in pts.windows(2) {
            let mid = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
            let sagitta = 10.0 - mid[0].hypot(mid[1]);
            assert!(sagitta <= tol + EPS, "tol {tol}: {sagitta}");
        }
    }
}

#[test]
fn finer_tolerance_gives_more_points() {
    let s = quarter_ccw();
    assert!(points(&s, 0.001).len() > points(&s, 0.1).len());
}

#[test]
fn full_circle_goes_all_the_way_round() {
    let s = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0],
        &[10.0, 0.0],
        &[0.0, 0.0],
        10.0,
    );
    let pts = points(&s, 0.01);
    assert_eq!(pts.first(), pts.last());
    assert!(pts.iter().any(|p| p[0] < -9.99));
}

#[test]
fn extra_turns_add_revolutions() {
    let one = quarter_ccw();
    let mut two = one.clone();
    if let Some(a) = two.arc.as_mut() {
        a.turns = 1;
    }
    // 90° vs 450°: five times as many chords.
    let (n1, n2) = (points(&one, 0.01).len() - 1, points(&two, 0.01).len() - 1);
    assert!((n2 as f64 / n1 as f64 - 5.0).abs() < 0.2, "{n1} {n2}");
}

#[test]
fn helix_moves_the_third_axis_evenly_with_the_angle() {
    // Full circle in X/Y descending 5 in Z.
    let s = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0, 0.0],
        &[10.0, 0.0, -5.0],
        &[0.0, 0.0],
        10.0,
    );
    let pts = points(&s, 0.01);
    let n = pts.len() - 1;
    for (i, p) in pts.iter().enumerate() {
        let angle = p[1].atan2(p[0]).rem_euclid(TAU);
        let expected_z = if i == n { -5.0 } else { -5.0 * angle / TAU };
        assert!((p[2] - expected_z).abs() < 1e-9, "{i}: {p:?}");
    }
}

#[test]
fn any_plane_works() {
    // Plane (Z, X), i.e. G18-style with axes 2 and 0; Y (axis 1) moves
    // linearly.
    let s = arc_in(
        SegKind::ArcCcw,
        &[0.0, 0.0, 10.0],
        &[10.0, 4.0, 0.0],
        &[0.0, 0.0, 0.0],
        10.0,
        ArcPlane {
            first: 2,
            second: 0,
        },
    );
    let pts = points(&s, 0.01);
    let c = pos(&[]);
    for p in &pts {
        assert!((dist(p, &c, 2, 0) - 10.0).abs() < EPS);
        assert!(p[0] >= -EPS && p[2] >= -EPS, "quarter from +Z toward +X");
    }
    assert!((pts[pts.len() / 2][1] - 2.0).abs() < 0.5);
}

#[test]
fn coarse_tolerance_still_gives_a_point_every_quarter_turn() {
    let s = arc(
        SegKind::ArcCw,
        &[10.0, 0.0],
        &[10.0, 0.0],
        &[0.0, 0.0],
        10.0,
    );
    assert_eq!(points(&s, 1000.0).len(), 5);
    for w in points(&s, 1000.0).windows(2) {
        let turned = (w[0][1].atan2(w[0][0]) - w[1][1].atan2(w[1][0])).rem_euclid(TAU);
        assert!(turned <= FRAC_PI_2 + EPS);
    }
}

#[test]
fn unusable_tolerance_gives_the_maximum_points_without_panicking() {
    let s = quarter_ccw();
    for tol in [0.0, -1.0, f64::NAN, f64::NEG_INFINITY] {
        assert_eq!(points(&s, tol).len(), MAX_ARC_POINTS, "{tol}");
    }
    // Infinity is a (very) coarse tolerance.
    assert_eq!(points(&s, f64::INFINITY).len(), 2);
}

#[test]
fn point_count_is_capped() {
    let mut s = quarter_ccw();
    if let Some(a) = s.arc.as_mut() {
        a.turns = u16::MAX;
    }
    let pts = points(&s, 1e-6);
    assert_eq!(pts.len(), MAX_ARC_POINTS);
    assert_eq!(pts.last(), Some(&s.end));
}

#[test]
fn unusable_arc_is_drawn_straight() {
    let mut bad_plane = quarter_ccw();
    if let Some(a) = bad_plane.arc.as_mut() {
        a.plane = ArcPlane {
            first: 1,
            second: 1,
        };
    }
    assert_eq!(
        points(&bad_plane, 0.01),
        vec![bad_plane.start, bad_plane.end]
    );

    let mut no_geometry = quarter_ccw();
    no_geometry.arc = None;
    assert_eq!(points(&no_geometry, 0.01).len(), 2);

    let mut far_plane = quarter_ccw();
    if let Some(a) = far_plane.arc.as_mut() {
        a.plane = ArcPlane {
            first: 0,
            second: 200,
        };
    }
    assert_eq!(points(&far_plane, 0.01).len(), 2);
}

#[test]
fn slightly_unequal_radii_blend_from_start_to_end() {
    // Start at radius 10, end at radius 10.005 (inside the interpreter's
    // 0.01 tolerance).
    let s = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0],
        &[0.0, 10.005],
        &[0.0, 0.0],
        10.0,
    );
    let pts = points(&s, 0.001);
    assert_eq!(pts.last(), Some(&s.end));
    let c = pos(&[]);
    for p in &pts {
        let r = dist(p, &c, 0, 1);
        assert!((10.0 - EPS..=10.005 + EPS).contains(&r), "{r}");
    }
}

proptest! {
    /// For any arc: the ends are exact, every point is on the arc, no
    /// chord is further than `tol` from it, the angle sums to the sweep,
    /// axes outside the plane move evenly, and every point is inside the
    /// segment's bounding box.
    #[test]
    fn arcs_tessellate_within_tolerance(
        cx in -1000.0f64..1000.0,
        cy in -1000.0f64..1000.0,
        r in 0.1f64..500.0,
        start_deg in 0.0f64..360.0,
        sweep_deg in 0.5f64..360.0,
        turns in 0u16..3,
        ccw in any::<bool>(),
        dz in -50.0f64..50.0,
        tol in 0.0005f64..2.0,
        plane_choice in 0usize..3,
    ) {
        // XY, ZX and YZ in the usual G17/G18/G19 orientation.
        let (a, b, z) = [(0, 1, 2), (2, 0, 1), (1, 2, 0)][plane_choice];
        let dir = if ccw { 1.0 } else { -1.0 };
        let sa = start_deg.to_radians();
        let sweep = sweep_deg.to_radians();
        let ea = sa + dir * sweep;
        let mut start = [0.0; ir::MAX_AXES];
        let mut end = [0.0; ir::MAX_AXES];
        let mut center = [0.0; ir::MAX_AXES];
        center[a] = cx;
        center[b] = cy;
        start[a] = cx + r * sa.cos();
        start[b] = cy + r * sa.sin();
        end[a] = cx + r * ea.cos();
        end[b] = cy + r * ea.sin();
        start[z] = 5.0;
        end[z] = 5.0 + dz;
        let kind = if ccw { SegKind::ArcCcw } else { SegKind::ArcCw };
        let plane = ArcPlane { first: a as u8, second: b as u8 };
        let mut s = seg(kind, start, end, 1);
        s.arc = Some(ir::ArcGeom { center, plane, radius: r, turns });

        let pts = points(&s, tol);
        prop_assert_eq!(pts.first(), Some(&s.start));
        prop_assert_eq!(pts.last(), Some(&s.end));
        let on_arc = 1e-9 * r.max(1.0) + 1e-9;
        let total = sweep + f64::from(turns) * TAU;
        let mut turned = 0.0;
        for p in &pts {
            prop_assert!((dist(p, &center, a, b) - r).abs() < on_arc * 1e3);
        }
        for w in pts.windows(2) {
            let mut mid = [0.0; ir::MAX_AXES];
            mid[a] = (w[0][a] + w[1][a]) / 2.0;
            mid[b] = (w[0][b] + w[1][b]) / 2.0;
            let sagitta = r - dist(&mid, &center, a, b);
            prop_assert!(sagitta <= tol + 1e-6, "sagitta {} > tol {}", sagitta, tol);
            let a0 = (w[0][b] - cy).atan2(w[0][a] - cx);
            let a1 = (w[1][b] - cy).atan2(w[1][a] - cx);
            let step = if ccw { (a1 - a0).rem_euclid(TAU) } else { (a0 - a1).rem_euclid(TAU) };
            // A step never exceeds 90°; a full-circle step can read as ~0.
            let step = if step > PI { step - TAU } else { step };
            prop_assert!(step <= FRAC_PI_2 + 1e-9);
            turned += step;
            // The third axis follows the angle evenly.
            let frac_z = (w[1][z] - start[z]) / dz;
            if dz.abs() > 1e-3 {
                prop_assert!((frac_z - turned / total).abs() < 1e-6, "{} vs {}", frac_z, turned / total);
            }
        }
        prop_assert!((turned - total).abs() < 1e-6, "turned {} of {}", turned, total);

        let bbox = bounding_box(std::slice::from_ref(&s)).expect("motion");
        for p in &pts {
            for i in [a, b, z] {
                prop_assert!(p[i] >= bbox.min[i] - 1e-6 && p[i] <= bbox.max[i] + 1e-6);
            }
        }
    }
}
