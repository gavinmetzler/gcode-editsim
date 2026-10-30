//! Arcs (manual p. 2-4): I and J are the centre on the plane's first and
//! second axes whatever the plane; in G90 they are a position, in G91 a
//! distance from the start. R is the radius, negative over 180 degrees.

mod common;

use common::{abs, inc, run};
use dialect_api::{ArcCenter, ArcDir, Command};
use ir::{ArcPlane, DiagCode};

const XY: ArcPlane = ArcPlane {
    first: 0,
    second: 1,
};

fn arcs(r: &common::Run) -> Vec<(&ArcCenter, ArcDir, ArcPlane, usize)> {
    r.commands
        .iter()
        .filter_map(|c| match c {
            Command::Arc {
                center,
                dir,
                plane,
                target,
            } => Some((center, *dir, *plane, target.len())),
            _ => None,
        })
        .collect()
}

#[test]
fn absolute_centre_in_g90() {
    // The manual's example: N15 G2 X46 Y20 I28 J20 F200.
    let r = run("G71 X10 Y20\nG2 X46 Y20 I28 J20 F200\nG3 X64 Y38 I46 J38\n");
    let a = arcs(&r);
    assert_eq!(a.len(), 2, "{:?}", r.diags);
    assert_eq!(
        a[0].0,
        &ArcCenter::Position(vec![abs(0, 28.0), abs(1, 20.0)])
    );
    assert_eq!((a[0].1, a[0].2), (ArcDir::Cw, XY));
    assert_eq!(
        a[1].0,
        &ArcCenter::Position(vec![abs(0, 46.0), abs(1, 38.0)])
    );
    assert_eq!(a[1].1, ArcDir::Ccw);
}

#[test]
fn incremental_centre_in_g91() {
    // N15 G2 G91 X36 I18 J0 F200 / N16 G3 X18 Y18 I0 J18.
    let r = run("G71 X10 Y20\nG2 G91 X36 I18 J0 F200\nG3 X18 Y18 I0 J18\n");
    let a = arcs(&r);
    assert_eq!(
        a[0].0,
        &ArcCenter::Position(vec![inc(0, 18.0), inc(1, 0.0)])
    );
    assert_eq!(
        a[1].0,
        &ArcCenter::Position(vec![inc(0, 0.0), inc(1, 18.0)])
    );
}

#[test]
fn incremental_centre_may_leave_out_a_zero_distance() {
    let r = run("G71 G91 G2 X36 I18 F200\n");
    assert_eq!(arcs(&r)[0].0, &ArcCenter::Position(vec![inc(0, 18.0)]));
}

#[test]
fn radius_form() {
    let r = run("G71 X10 Y20\nG2 X46 Y20 R18 F200\nG3 X64 Y38 R-18\n");
    let a = arcs(&r);
    assert_eq!(a[0].0, &ArcCenter::Radius(18.0));
    assert_eq!(a[1].0, &ArcCenter::Radius(-18.0));
}

#[test]
fn no_axes_is_a_full_circle() {
    let r = run("G71 G2 I28 J20 F200\n");
    let a = arcs(&r);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].3, 0, "no end point: the interpreter closes the circle");
}

#[test]
fn radius_cannot_make_a_full_circle() {
    let r = run("G71 G2 R18 F200\n");
    assert!(arcs(&r).is_empty());
    assert!(r.has(DiagCode::InvalidArc, "full circle"));
    assert!(
        !r.position_lost(),
        "the control refuses it, so nothing moved"
    );
}

#[test]
fn absolute_centre_needs_both_i_and_j() {
    let r = run("G71 G2 X46 Y20 I28 F200\n");
    assert!(arcs(&r).is_empty());
    assert!(r.has(DiagCode::MissingWord, "both I and J"));
    assert!(r.position_lost());
}

#[test]
fn helix_pitch_is_not_simulated() {
    let r = run("G71 G2 X46 Y20 Z-5 I28 J20 K2 F200\n");
    assert!(arcs(&r).is_empty());
    assert!(r.has(DiagCode::NotYetSimulated, "helix pitch K"));
    assert!(r.position_lost());
}

#[test]
fn i_and_j_follow_the_plane_not_the_axis_letters() {
    // In G18 (axes 3 and 1: Z then X) I is Z and J is X.
    let r = run("G18\nG71 G2 X10 Z0 I5 J0 F100\n");
    let a = arcs(&r);
    assert_eq!(a[0].0, &ArcCenter::Position(vec![abs(2, 5.0), abs(0, 0.0)]));
    assert_eq!(
        a[0].2,
        ArcPlane {
            first: 2,
            second: 0
        }
    );
}

#[test]
fn inch_centres_are_converted() {
    let r = run("G70 G2 X1 Y0 I0.5 J0 F10\n");
    assert_eq!(
        arcs(&r)[0].0,
        &ArcCenter::Position(vec![abs(0, 12.7), abs(1, 0.0)])
    );
}
