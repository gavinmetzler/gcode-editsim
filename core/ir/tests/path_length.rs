//! `path_length`: how far a move travels, along the arc for arcs, over the
//! axes the caller counts (the time estimate counts linear axes only).

mod common;

use std::f64::consts::PI;

use common::{arc, line_seg, pos, seg};
use ir::{path_length, SegKind};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} vs {b}");
}

#[test]
fn a_straight_move_is_its_distance() {
    close(
        path_length(&line_seg(&[0.0, 0.0, 0.0], &[3.0, 4.0, 12.0]), |_| true),
        13.0,
    );
}

#[test]
fn uncounted_axes_are_left_out() {
    let s = line_seg(&[0.0, 0.0, 0.0, 0.0], &[3.0, 4.0, 0.0, 90.0]);
    close(path_length(&s, |k| k < 3), 5.0);
}

#[test]
fn an_arc_is_measured_along_the_arc() {
    // Quarter circle, radius 10: 5π.
    let q = arc(
        SegKind::ArcCcw,
        &[10.0, 0.0],
        &[0.0, 10.0],
        &[0.0, 0.0],
        10.0,
    );
    close(path_length(&q, |_| true), 5.0 * PI);
    // The same end points clockwise go three quarters of the way round.
    let r = arc(
        SegKind::ArcCw,
        &[10.0, 0.0],
        &[0.0, 10.0],
        &[0.0, 0.0],
        10.0,
    );
    close(path_length(&r, |_| true), 15.0 * PI);
}

#[test]
fn a_helix_combines_the_arc_and_the_climb() {
    // A full circle of radius 1 (2π) that climbs 2π along axis 2.
    let h = arc(
        SegKind::ArcCcw,
        &[1.0, 0.0, 0.0],
        &[1.0, 0.0, 2.0 * PI],
        &[0.0, 0.0],
        1.0,
    );
    close(path_length(&h, |_| true), 2.0 * PI * 2f64.sqrt());
}

#[test]
fn events_and_unusable_values_have_no_length() {
    let e = seg(SegKind::Dwell, pos(&[1.0]), pos(&[1.0]), 1);
    close(path_length(&e, |_| true), 0.0);
    let bad = line_seg(&[0.0], &[f64::NAN]);
    assert!(path_length(&bad, |_| true).is_nan());
}

mod property {
    use super::*;
    use ir::{tessellate, ArcGeom, ArcPlane};
    use proptest::prelude::*;

    proptest! {
        /// An arc's length is a little more than the chords drawn for it,
        /// and the chords close in on it as the tolerance shrinks.
        #[test]
        fn arc_length_bounds_its_chords(
            r in 0.5f64..200.0,
            start_deg in 0.0f64..360.0,
            sweep_deg in 1.0f64..359.0,
            ccw in any::<bool>(),
            dz in -20.0f64..20.0,
        ) {
            let dir = if ccw { 1.0 } else { -1.0 };
            let sa = start_deg.to_radians();
            let ea = sa + dir * sweep_deg.to_radians();
            let mut s = seg(
                if ccw { SegKind::ArcCcw } else { SegKind::ArcCw },
                pos(&[r * sa.cos(), r * sa.sin(), 0.0]),
                pos(&[r * ea.cos(), r * ea.sin(), dz]),
                1,
            );
            s.arc = Some(ArcGeom {
                center: pos(&[0.0, 0.0]),
                plane: ArcPlane { first: 0, second: 1 },
                radius: r,
                turns: 0,
            });
            let length = path_length(&s, |_| true);
            let chords = |tol: f64| {
                let pts: Vec<_> = tessellate(&s, tol).collect();
                pts.windows(2)
                    .map(|w| (0..3).map(|k| (w[1][k] - w[0][k]).powi(2)).sum::<f64>().sqrt())
                    .sum::<f64>()
            };
            let fine = chords(r * 1e-6);
            prop_assert!(fine <= length + 1e-6, "{fine} > {length}");
            prop_assert!((length - fine) / length < 1e-4, "{fine} vs {length}");
        }
    }
}
