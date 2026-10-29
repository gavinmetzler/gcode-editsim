//! G2/G3 with I/J/K centre offsets.

mod common;

use common::{run, w};
use dialect_api::{ArcCenter, ArcDir, AxisOffset, Command};
use ir::{ArcPlane, DiagCode, Severity};

const XY: ArcPlane = ArcPlane {
    first: 0,
    second: 1,
};

fn off(axis: usize, value: f64) -> AxisOffset {
    AxisOffset { axis, value }
}

#[test]
fn clockwise_and_counter_clockwise() {
    let r = run("F100.\nG2 X10. Y0 I5. J0\nG3 X0 Y0 I-5.\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    assert_eq!(
        r.moves(),
        vec![
            &Command::Arc {
                target: vec![w(0, 10.0), w(1, 0.0)],
                center: ArcCenter::Offset(vec![off(0, 5.0), off(1, 0.0)]),
                dir: ArcDir::Cw,
                plane: XY,
            },
            &Command::Arc {
                target: vec![w(0, 0.0), w(1, 0.0)],
                center: ArcCenter::Offset(vec![off(0, -5.0)]),
                dir: ArcDir::Ccw,
                plane: XY,
            },
        ]
    );
}

#[test]
fn offsets_alone_give_a_full_circle() {
    // No end point: the arc ends where it starts.
    let r = run("G2 I10. F100.\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Arc {
            target: vec![],
            center: ArcCenter::Offset(vec![off(0, 10.0)]),
            dir: ArcDir::Cw,
            plane: XY,
        }]
    );
}

#[test]
fn arc_mode_persists_and_needs_centre_each_block() {
    let r = run("G2 X10. I5. F100.\nX20. I5.\nX30.\n");
    assert_eq!(r.moves().len(), 2);
    assert_eq!(r.codes(), vec![DiagCode::InvalidArc]);
    assert_eq!(r.diags[0].src.line, 3);
}

#[test]
fn helix_keeps_the_third_axis_as_a_target() {
    let r = run("G3 X0 Y10. Z-2. I-10. F50.\n");
    let Command::Arc { target, .. } = r.moves()[0] else {
        panic!()
    };
    assert_eq!(target, &vec![w(0, 0.0), w(1, 10.0), w(2, -2.0)]);
}

#[test]
fn offset_outside_the_plane_is_ignored_with_a_warning() {
    let r = run("G2 X10. I5. K3. F100.\n");
    assert_eq!(r.codes(), vec![DiagCode::InvalidArc]);
    assert_eq!(r.diags[0].severity, Severity::Warning);
    let Command::Arc { center, .. } = r.moves()[0] else {
        panic!()
    };
    assert_eq!(center, &ArcCenter::Offset(vec![off(0, 5.0)]));
}

#[test]
fn only_out_of_plane_offsets_is_an_error() {
    let r = run("G2 X10. K3. F100.\n");
    assert!(r.moves().is_empty());
    assert_eq!(r.errors().len(), 1);
    assert_eq!(r.errors()[0].code, DiagCode::InvalidArc);
}

#[test]
fn missing_centre_is_an_error() {
    let r = run("G2 X10. Y0 F100.\n");
    assert!(r.moves().is_empty());
    assert_eq!(r.codes(), vec![DiagCode::InvalidArc]);
    assert_eq!(r.diags[0].severity, Severity::Error);
}
