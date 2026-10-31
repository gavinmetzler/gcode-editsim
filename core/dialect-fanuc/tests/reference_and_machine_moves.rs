//! G28 (reference return), G30 (other reference positions) and G53
//! (machine coordinates).

mod common;

use common::{inc, run, w};
use dialect_api::Command;
use ir::DiagCode;

#[test]
fn g28_returns_via_the_intermediate_point_in_the_current_mode() {
    let r = run("G91 G28 Z0.\nG90 G28 X10. Y20.\n");
    assert_eq!(
        r.moves_and_returns(),
        vec![
            &Command::ReturnToReference {
                via: vec![inc(2, 0.0)],
                position: 1
            },
            &Command::ReturnToReference {
                via: vec![w(0, 10.0), w(1, 20.0)],
                position: 1
            },
        ]
    );
    assert!(r.errors().is_empty(), "{:?}", r.diags);
}

#[test]
fn g28_without_axes_does_nothing() {
    let r = run("G28\n");
    assert!(r.commands.is_empty());
}

#[test]
fn g28_does_not_use_the_motion_mode() {
    // G28 is one-shot positioning; the G01 mode and missing F do not matter,
    // and G01 stays active afterwards.
    let r = run("G90 G1 G28 Z5.\nX1. F100.\n");
    assert!(r.errors().is_empty(), "{:?}", r.diags);
    assert!(matches!(r.commands.last(), Some(Command::Linear { .. })));
}

#[test]
fn g30_returns_to_the_numbered_reference_position() {
    for (src, position) in [
        ("G91 G30 Z0.\n", 2),
        ("G91 G30 P3 Z0.\n", 3),
        ("G91 G30 P4 Z0.\n", 4),
    ] {
        let r = run(src);
        assert_eq!(
            r.commands.last(),
            Some(&Command::ReturnToReference {
                via: vec![inc(2, 0.0)],
                position
            }),
            "{src}"
        );
        assert!(r.diags.is_empty(), "{src}: {:?}", r.diags);
    }
}

#[test]
fn g53_is_a_machine_coordinate_rapid() {
    let r = run("G90 G53 Z-100.\n");
    assert_eq!(
        r.commands,
        vec![
            Command::SetDistanceMode(dialect_api::DistanceMode::Absolute),
            Command::MachineRapid {
                target: vec![w(2, -100.0)]
            }
        ]
    );
}

#[test]
fn g53_is_ignored_in_incremental_mode() {
    let r = run("G91 G53 Z-100.\n");
    assert!(!r
        .commands
        .iter()
        .any(|c| matches!(c, Command::MachineRapid { .. })));
    assert_eq!(r.codes(), vec![DiagCode::ControllerDependent]);
}

#[test]
fn reference_return_still_runs_after_an_ignored_mode() {
    // Moves are never hidden (owner decision, 2026-10-31): after an ignored
    // rotation, G28 still returns to the reference position.
    let r = run("G68 X0 Y0 R45.\nG91 G28 Z0.\n");
    assert!(r
        .commands
        .iter()
        .any(|c| matches!(c, Command::ReturnToReference { .. })));
    assert!(!r
        .commands
        .iter()
        .any(|c| matches!(c, Command::PositionUnknown { .. })));
}
