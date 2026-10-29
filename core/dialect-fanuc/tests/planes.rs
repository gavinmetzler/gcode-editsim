//! G17/G18/G19 select the arc plane from the machine's own axes.

mod common;

use common::{axes, run, run_with};
use dialect_api::{ArcCenter, AxisOffset, Command};
use dialect_fanuc::FanucOptions;
use ir::{ArcPlane, DiagCode};

#[test]
fn plane_selection_commands() {
    let r = run("G18\nG19\nG17\n");
    assert_eq!(
        r.commands,
        vec![
            Command::SetPlane(ArcPlane {
                first: 2,
                second: 0
            }),
            Command::SetPlane(ArcPlane {
                first: 1,
                second: 2
            }),
            Command::SetPlane(ArcPlane {
                first: 0,
                second: 1
            }),
        ]
    );
}

#[test]
fn arcs_use_the_selected_plane_and_its_offsets() {
    let r = run("G18 G2 X10. Z0 I5. K0 F100.\n");
    let Command::Arc { plane, center, .. } = r.moves()[0] else {
        panic!()
    };
    assert_eq!(
        *plane,
        ArcPlane {
            first: 2,
            second: 0
        }
    );
    assert_eq!(
        center,
        &ArcCenter::Offset(vec![
            AxisOffset {
                axis: 0,
                value: 5.0
            },
            AxisOffset {
                axis: 2,
                value: 0.0
            }
        ])
    );
}

#[test]
fn axis_order_follows_the_machine_not_the_letters() {
    // A machine listing its axes as Z, X, Y: G17 still means X then Y.
    let machine = axes(&[
        ('Z', Some('K'), false),
        ('X', Some('I'), false),
        ('Y', Some('J'), false),
    ]);
    let r = run_with(FanucOptions::default(), &machine, "G17\n");
    assert_eq!(
        r.commands,
        vec![Command::SetPlane(ArcPlane {
            first: 1,
            second: 2
        })]
    );
}

#[test]
fn plane_needs_both_axes_on_the_machine() {
    let machine = axes(&[('X', Some('I'), false), ('Z', Some('K'), false)]);
    let r = run_with(FanucOptions::default(), &machine, "G17\nG2 X1. I1. F100.\n");
    assert_eq!(
        r.codes(),
        vec![DiagCode::InvalidValue, DiagCode::InvalidArc]
    );
    assert!(r.moves().is_empty());
    // G18 works on this machine.
    let r = run_with(FanucOptions::default(), &machine, "G18 G2 X1. I1. F100.\n");
    assert_eq!(r.moves().len(), 1);
}
