//! Arc centres and radii, worked out in machine coordinates.

mod common;

use common::{at, errors, moves, run};
use ir::{ArcPlane, DiagCode, SegKind};

/// Program prefix: G54 origin is X-380 Y-205, and we start at part X0 Y0.
const START: &str = "G0 X0 Y0 Z0\nF100.\n";

fn center(p: &ir::Program, i: usize) -> (f64, f64, f64) {
    let a = moves(p)[i].arc.expect("arc");
    (a.center[0], a.center[1], a.radius)
}

fn close(a: (f64, f64, f64), b: (f64, f64, f64)) {
    assert!(
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9 && (a.2 - b.2).abs() < 1e-9,
        "{a:?} != {b:?}"
    );
}

#[test]
fn centre_offsets_are_relative_to_the_start() {
    let p = run(&format!("{START}G2 X20. Y0 I10. J0\n"));
    let m = moves(&p);
    assert_eq!(m[1].kind, SegKind::ArcCw);
    at(&m[1].end, [-360.0, -205.0, -400.0]);
    close(center(&p, 1), (-370.0, -205.0, 10.0));
    assert_eq!(
        m[1].arc.unwrap().plane,
        ArcPlane {
            first: 0,
            second: 1
        }
    );
}

#[test]
fn full_circle_from_offsets_alone() {
    let p = run(&format!("{START}G3 I5.\n"));
    let m = moves(&p);
    assert_eq!(m[1].start, m[1].end);
    close(center(&p, 1), (-375.0, -205.0, 5.0));
}

#[test]
fn radius_picks_the_correct_centre_for_each_direction_and_sign() {
    // From part (0,0) to (10,10) with |R| = 10, the two possible centres
    // are part (10,0) and (0,10).
    let cases = [
        // CCW short arc turns about (0,10): from -90 deg to 0 deg.
        ("G3 X10. Y10. R10.", (0.0, 10.0)),
        // CW short arc turns about (10,0): from 180 deg down to 90 deg.
        ("G2 X10. Y10. R10.", (10.0, 0.0)),
        // Negative R takes the other centre (the arc longer than 180 deg).
        ("G3 X10. Y10. R-10.", (10.0, 0.0)),
        ("G2 X10. Y10. R-10.", (0.0, 10.0)),
    ];
    for (block, (cx, cy)) in cases {
        let p = run(&format!("{START}{block}\n"));
        assert!(errors(&p).is_empty(), "{block}: {:?}", p.diagnostics);
        close(center(&p, 1), (-380.0 + cx, -205.0 + cy, 10.0));
    }
}

#[test]
fn half_circle_radius() {
    let p = run(&format!("{START}G2 X20. Y0 R10.\n"));
    close(center(&p, 1), (-370.0, -205.0, 10.0));
}

#[test]
fn radius_too_small_is_an_error() {
    let p = run(&format!("{START}G2 X20. Y0 R5.\n"));
    assert_eq!(errors(&p), vec![DiagCode::InvalidArc]);
    assert_eq!(moves(&p).len(), 1);
}

#[test]
fn radius_with_coincident_end_points_is_an_error() {
    let p = run(&format!("{START}G2 X0 Y0 R5.\n"));
    assert_eq!(errors(&p), vec![DiagCode::InvalidArc]);
}

#[test]
fn end_point_off_the_circle_is_rejected_and_position_kept() {
    let p = run(&format!("{START}G2 X20. Y0 I10.5 J0\nG1 X1.\n"));
    assert_eq!(errors(&p), vec![DiagCode::ArcRadiusMismatch]);
    let m = moves(&p);
    // The rejected arc is not drawn; the next move starts where we were.
    assert_eq!(m.len(), 2);
    at(&m[1].start, [-380.0, -205.0, -400.0]);
}

#[test]
fn small_mismatch_within_tolerance_is_accepted() {
    let p = run(&format!("{START}G2 X20.005 Y0 I10. J0\n"));
    assert!(errors(&p).is_empty());
}

#[test]
fn helix_moves_the_third_axis_linearly() {
    let p = run(&format!("{START}G3 X0 Y0 Z-5. I10.\n"));
    let m = moves(&p);
    at(&m[1].end, [-380.0, -205.0, -405.0]);
    close(center(&p, 1), (-370.0, -205.0, 10.0));
}

#[test]
fn zx_plane_arc() {
    let p = run(&format!("{START}G18 G2 X10. Z-10. R10.\n"));
    let arc = moves(&p)[1].arc.unwrap();
    assert_eq!(
        arc.plane,
        ArcPlane {
            first: 2,
            second: 0
        }
    );
    assert!((arc.radius - 10.0).abs() < 1e-9);
}
