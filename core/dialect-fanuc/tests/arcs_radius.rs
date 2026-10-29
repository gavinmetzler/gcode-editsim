//! G2/G3 with R.

mod common;

use common::{run, w};
use dialect_api::{ArcCenter, ArcDir, Command};
use ir::{ArcPlane, DiagCode, Severity};

#[test]
fn radius_arc() {
    let r = run("G3 X10. Y10. R10. F100.\n");
    assert!(r.diags.is_empty());
    assert_eq!(
        r.moves(),
        vec![&Command::Arc {
            target: vec![w(0, 10.0), w(1, 10.0)],
            center: ArcCenter::Radius(10.0),
            dir: ArcDir::Ccw,
            plane: ArcPlane {
                first: 0,
                second: 1
            },
        }]
    );
}

#[test]
fn negative_radius_is_kept_for_the_long_arc() {
    let r = run("G2 X10. R-10. F100.\n");
    let Command::Arc { center, .. } = r.moves()[0] else {
        panic!()
    };
    assert_eq!(center, &ArcCenter::Radius(-10.0));
}

#[test]
fn radius_wins_over_offsets_with_a_warning() {
    let r = run("G2 X10. R5. I3. F100.\n");
    assert_eq!(r.codes(), vec![DiagCode::ControllerDependent]);
    assert_eq!(r.diags[0].severity, Severity::Warning);
    let Command::Arc { center, .. } = r.moves()[0] else {
        panic!()
    };
    assert_eq!(center, &ArcCenter::Radius(5.0));
}

#[test]
fn radius_without_end_point_does_not_move() {
    // Fanuc treats this as an arc of 0 degrees.
    let r = run("G2 R5. F100.\n");
    assert!(r.moves().is_empty());
    assert_eq!(r.codes(), vec![DiagCode::InvalidArc]);
    assert_eq!(r.diags[0].severity, Severity::Warning);
}
