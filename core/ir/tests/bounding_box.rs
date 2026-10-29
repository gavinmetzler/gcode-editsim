//! Tests for `bounding_box`, especially arcs that bulge past their end points.

mod common;

use std::f64::consts::PI;

use common::{arc, arc_in, line_seg, pos, seg};
use ir::{bounding_box, ArcPlane, BoundingBox, Pos, SegKind, Segment};
use proptest::prelude::*;

const EPS: f64 = 1e-9;

fn assert_axis(b: &BoundingBox, axis: usize, min: f64, max: f64) {
    assert!(
        (b.min[axis] - min).abs() < EPS && (b.max[axis] - max).abs() < EPS,
        "axis {axis}: got {}..{}, expected {min}..{max}",
        b.min[axis],
        b.max[axis]
    );
}

fn bbox(segs: &[Segment]) -> BoundingBox {
    bounding_box(segs).expect("has motion")
}

fn polar(c: (f64, f64), r: f64, deg: f64) -> [f64; 2] {
    let a = deg.to_radians();
    [c.0 + r * a.cos(), c.1 + r * a.sin()]
}

#[test]
fn no_segments_gives_none() {
    assert_eq!(bounding_box(&[]), None);
}

#[test]
fn non_motion_segments_are_ignored() {
    let far = pos(&[1000.0, 1000.0]);
    let segs = [
        seg(SegKind::Dwell, far, far, 1),
        seg(SegKind::ToolChange, far, far, 2),
        seg(SegKind::Sync, far, far, 3),
    ];
    assert_eq!(bounding_box(&segs), None);

    let mut with_motion = segs.to_vec();
    with_motion.push(line_seg(&[0.0, 0.0], &[1.0, 2.0]));
    let b = bbox(&with_motion);
    assert_axis(&b, 0, 0.0, 1.0);
    assert_axis(&b, 1, 0.0, 2.0);
}

#[test]
fn linear_and_rapid_segments_use_end_points() {
    let segs = [
        seg(
            SegKind::Rapid,
            pos(&[0.0, 0.0, 50.0]),
            pos(&[-5.0, 3.0, 50.0]),
            1,
        ),
        line_seg(&[-5.0, 3.0, 50.0], &[20.0, -7.0, -2.5]),
    ];
    let b = bbox(&segs);
    assert_axis(&b, 0, -5.0, 20.0);
    assert_axis(&b, 1, -7.0, 3.0);
    assert_axis(&b, 2, -2.5, 50.0);
    // Unused axes stay at zero.
    assert_axis(&b, 8, 0.0, 0.0);
}

#[test]
fn caller_can_filter_segments() {
    let segs = [
        seg(
            SegKind::Rapid,
            pos(&[0.0, 0.0, 100.0]),
            pos(&[0.0, 0.0, 5.0]),
            1,
        ),
        line_seg(&[0.0, 0.0, 5.0], &[0.0, 0.0, -1.0]),
    ];
    let cutting = bounding_box(segs.iter().filter(|s| s.kind != SegKind::Rapid)).unwrap();
    assert_axis(&cutting, 2, -1.0, 5.0);
}

#[test]
fn quarter_arc_without_bulge() {
    // CCW from 0° to 90°: the extremes are the end points themselves.
    let s = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0],
        &[0.0, 10.0],
        &[0.0, 0.0],
        10.0,
    );
    let b = bbox(&[s]);
    assert_axis(&b, 0, 0.0, 10.0);
    assert_axis(&b, 1, 0.0, 10.0);
}

#[test]
fn small_arc_bulging_past_both_end_points() {
    // CCW from 80° to 100° around (5, 5), r = 10. Both end points sit at
    // y ≈ 14.85, but the arc passes over the top of the circle at y = 15.
    let c = (5.0, 5.0);
    let s = arc(
        SegKind::ArcCcw,
        &polar(c, 10.0, 80.0),
        &polar(c, 10.0, 100.0),
        &[c.0, c.1],
        10.0,
    );
    let end_y = polar(c, 10.0, 80.0)[1];
    assert!(end_y < 14.9);
    let b = bbox(&[s]);
    assert_axis(&b, 1, end_y, 15.0);
    assert_axis(&b, 0, polar(c, 10.0, 100.0)[0], polar(c, 10.0, 80.0)[0]);
}

#[test]
fn same_end_points_opposite_directions() {
    // Semicircle from (10, 0) to (-10, 0): CCW goes over the top, CW under.
    let ccw = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0],
        &[-10.0, 0.0],
        &[0.0, 0.0],
        10.0,
    );
    let b = bbox(&[ccw]);
    assert_axis(&b, 0, -10.0, 10.0);
    assert_axis(&b, 1, 0.0, 10.0);

    let cw = arc(
        SegKind::ArcCw,
        &[10.0, 0.0],
        &[-10.0, 0.0],
        &[0.0, 0.0],
        10.0,
    );
    let b = bbox(&[cw]);
    assert_axis(&b, 0, -10.0, 10.0);
    assert_axis(&b, 1, -10.0, 0.0);
}

#[test]
fn cw_arc_the_long_way_round() {
    // CW from 0° to 90° sweeps 270° through -90° and 180°.
    let s = arc(
        SegKind::ArcCw,
        &[10.0, 0.0],
        &[0.0, 10.0],
        &[0.0, 0.0],
        10.0,
    );
    let b = bbox(&[s]);
    assert_axis(&b, 0, -10.0, 10.0);
    assert_axis(&b, 1, -10.0, 10.0);
}

#[test]
fn coincident_end_points_are_a_full_circle() {
    for kind in [SegKind::ArcCw, SegKind::ArcCcw] {
        let s = arc(kind, &[12.0, 3.0], &[12.0, 3.0], &[7.0, 3.0], 5.0);
        let b = bbox(&[s]);
        assert_axis(&b, 0, 2.0, 12.0);
        assert_axis(&b, 1, -2.0, 8.0);
    }
}

#[test]
fn extra_turns_cover_the_whole_circle() {
    let mut s = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0],
        &[0.0, 10.0],
        &[0.0, 0.0],
        10.0,
    );
    if let Some(a) = s.arc.as_mut() {
        a.turns = 1;
    }
    let b = bbox(&[s]);
    assert_axis(&b, 0, -10.0, 10.0);
    assert_axis(&b, 1, -10.0, 10.0);
}

#[test]
fn extreme_at_an_end_point_is_not_double_counted() {
    // CCW from 90° to 180°: starts exactly on the top extreme.
    let s = arc(
        SegKind::ArcCcw,
        &[0.0, 10.0],
        &[-10.0, 0.0],
        &[0.0, 0.0],
        10.0,
    );
    let b = bbox(&[s]);
    assert_axis(&b, 0, -10.0, 0.0);
    assert_axis(&b, 1, 0.0, 10.0);
}

#[test]
fn arc_in_another_plane() {
    // Plane (axis 2, axis 0): counter-clockwise turns from axis 2 toward
    // axis 0. Start on +axis2, end on -axis2, so it passes through +axis0.
    let plane = ArcPlane {
        first: 2,
        second: 0,
    };
    let s = arc_in(
        SegKind::ArcCcw,
        &[0.0, 7.0, 4.0],
        &[0.0, 7.0, -4.0],
        &[0.0, 0.0, 0.0],
        4.0,
        plane,
    );
    let b = bbox(&[s]);
    assert_axis(&b, 2, -4.0, 4.0);
    assert_axis(&b, 0, 0.0, 4.0);
    assert_axis(&b, 1, 7.0, 7.0);
}

#[test]
fn helix_bounds_the_out_of_plane_axis_by_its_end_points() {
    let s = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0, 0.0],
        &[-10.0, 0.0, -3.0],
        &[0.0, 0.0],
        10.0,
    );
    let b = bbox(&[s]);
    assert_axis(&b, 1, 0.0, 10.0);
    assert_axis(&b, 2, -3.0, 0.0);
}

#[test]
fn malformed_arc_data_falls_back_to_end_points() {
    let bad_planes = [
        ArcPlane {
            first: 0,
            second: 0,
        },
        ArcPlane {
            first: 0,
            second: 9,
        },
        ArcPlane {
            first: 255,
            second: 1,
        },
    ];
    for plane in bad_planes {
        let s = arc_in(
            SegKind::ArcCcw,
            &[10.0, 0.0],
            &[-10.0, 0.0],
            &[0.0, 0.0],
            10.0,
            plane,
        );
        let b = bbox(&[s]);
        assert_axis(&b, 1, 0.0, 0.0);
    }
    for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let s = arc(
            SegKind::ArcCcw,
            &[10.0, 0.0],
            &[-10.0, 0.0],
            &[0.0, 0.0],
            radius,
        );
        let b = bbox(&[s]);
        assert_axis(&b, 1, 0.0, 0.0);
    }
    // Arc kind without arc data.
    let mut s = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0],
        &[-10.0, 0.0],
        &[0.0, 0.0],
        10.0,
    );
    s.arc = None;
    assert_axis(&bbox(&[s]), 1, 0.0, 0.0);
}

#[test]
fn arc_data_on_a_linear_segment_is_ignored() {
    let mut s = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0],
        &[-10.0, 0.0],
        &[0.0, 0.0],
        10.0,
    );
    s.kind = SegKind::Linear;
    assert_axis(&bbox(&[s]), 1, 0.0, 0.0);
}

#[test]
fn combines_several_segments() {
    let segs = [
        line_seg(&[-20.0, 0.0], &[10.0, 0.0]),
        arc(
            SegKind::ArcCcw,
            &[10.0, 0.0],
            &[-10.0, 0.0],
            &[0.0, 0.0],
            10.0,
        ),
        line_seg(&[-10.0, 0.0], &[-10.0, -30.0]),
    ];
    let b = bbox(&segs);
    assert_axis(&b, 0, -20.0, 10.0);
    assert_axis(&b, 1, -30.0, 10.0);
}

/// A point on the arc at fraction `t` of the way from start to end.
fn point_on_arc(s: &Segment, sweep: f64, t: f64) -> Pos {
    let a = s.arc.as_ref().unwrap();
    let (c0, c1) = (a.center[0], a.center[1]);
    let start = (s.start[1] - c1).atan2(s.start[0] - c0);
    let dir = if s.kind == SegKind::ArcCcw { 1.0 } else { -1.0 };
    let angle = start + dir * sweep * t;
    let mut p = s.start;
    p[0] = c0 + a.radius * angle.cos();
    p[1] = c1 + a.radius * angle.sin();
    p
}

proptest! {
    /// The box contains every point of the arc and is tight: each face is
    /// touched by some point on the arc.
    #[test]
    fn arc_box_contains_arc_and_is_tight(
        cx in -100.0f64..100.0,
        cy in -100.0f64..100.0,
        r in 0.1f64..50.0,
        start_deg in 0.0f64..360.0,
        sweep_deg in 1.0f64..359.0,
        ccw in any::<bool>(),
    ) {
        let sweep = sweep_deg.to_radians();
        let start_a = start_deg.to_radians();
        let dir = if ccw { 1.0 } else { -1.0 };
        let end_a = start_a + dir * sweep;
        let kind = if ccw { SegKind::ArcCcw } else { SegKind::ArcCw };
        let s = arc(
            kind,
            &[cx + r * start_a.cos(), cy + r * start_a.sin()],
            &[cx + r * end_a.cos(), cy + r * end_a.sin()],
            &[cx, cy],
            r,
        );
        let b = bbox(std::slice::from_ref(&s));

        let n = 4000;
        let mut seen = BoundingBox::from_point(&s.start);
        for i in 0..=n {
            let p = point_on_arc(&s, sweep, f64::from(i) / f64::from(n));
            for ((v, lo), hi) in p.iter().zip(&b.min).zip(&b.max).take(2) {
                prop_assert!(*v >= lo - 1e-9 && *v <= hi + 1e-9);
            }
            seen.include(&p);
        }
        // Dense sampling gets within a tiny chord error of each true face.
        let tol = r * (1.0 - (PI / f64::from(n)).cos()) + 1e-9;
        for axis in 0..2 {
            prop_assert!((seen.min[axis] - b.min[axis]).abs() <= tol);
            prop_assert!((seen.max[axis] - b.max[axis]).abs() <= tol);
        }
    }
}
