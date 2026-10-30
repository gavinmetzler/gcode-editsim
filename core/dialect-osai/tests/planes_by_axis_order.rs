//! G17, G18 and G19 select the plane of the 1st and 2nd, 3rd and 1st, and
//! 2nd and 3rd axes as configured, not axes by letter (manual p. 2-47).
//! They must be alone in their block (compatibility table, p. 1-10).

mod common;

use common::{axes, run, run_on};
use dialect_api::Command;
use ir::{ArcPlane, DiagCode};

fn planes(r: &common::Run) -> Vec<ArcPlane> {
    r.commands
        .iter()
        .filter_map(|c| match c {
            Command::SetPlane(p) => Some(*p),
            _ => None,
        })
        .collect()
}

#[test]
fn planes_by_position() {
    let r = run("G18\nG19\nG17\n");
    assert_eq!(
        planes(&r),
        vec![
            ArcPlane {
                first: 2,
                second: 0
            },
            ArcPlane {
                first: 1,
                second: 2
            },
            ArcPlane {
                first: 0,
                second: 1
            },
        ]
    );
}

#[test]
fn axis_order_other_than_xyz_is_pointed_out() {
    let ax = axes(&[('X', false), ('Z', false), ('Y', false)]);
    let r = run_on("G17\nG18\n", &ax);
    assert_eq!(
        planes(&r)[0],
        ArcPlane {
            first: 0,
            second: 1
        }
    );
    let notes = r
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::ControllerDependent && d.message.contains("XZY"))
        .count();
    assert_eq!(notes, 1);
}

#[test]
fn missing_third_axis() {
    let r = run_on("G18\n", &axes(&[('X', false), ('Z', false)]));
    assert!(planes(&r).is_empty());
    assert!(r.has(DiagCode::InvalidValue, "axes 3 and 1"));
}

#[test]
fn plane_code_must_be_alone() {
    let r = run("G17 G1 X10 F100\n");
    assert!(r.has(DiagCode::ModalGroupConflict, "must be alone"));
    assert!(r.commands.is_empty());
}

#[test]
fn g16_with_axis_names_leaves_no_plane_for_arcs() {
    // The parser cannot read "G16 X A"; the plane is then unknown.
    let r = run("G16 X Z\nG71 G2 X10 I5 J0 F100\nG17\nG2 X20 I15 J0\n");
    assert!(r.has(DiagCode::UnsupportedGCode, "G16"));
    assert!(r.has(DiagCode::InvalidArc, "no arc plane"));
    let arcs = r
        .commands
        .iter()
        .filter(|c| matches!(c, Command::Arc { .. }))
        .count();
    assert_eq!(arcs, 1, "only the arc after G17");
}
