//! M5.6b: a machine file that states the control's power-on state means
//! the program may rely on it: no "assumed ..." warnings, and the stated
//! modes are the ones used.

mod common;

use common::{run, run_power_on, target};
use dialect_api::{DistanceMode, PowerOn, StartMotion};
use ir::LengthUnit;

const PROGRAM: &str = "X10. Y10.\nG2 X20. R5. F100.\n";

fn stated() -> PowerOn {
    PowerOn {
        units: Some(LengthUnit::Metric),
        plane: Some(('X', 'Y')),
        motion: Some(StartMotion::Rapid),
        distance: Some(DistanceMode::Absolute),
    }
}

#[test]
fn without_a_stated_power_on_state_the_assumptions_are_warned_about() {
    assert!(!run(PROGRAM).assumed.is_empty());
}

#[test]
fn a_stated_power_on_state_gives_no_assumed_warnings() {
    let r = run_power_on(&stated(), PROGRAM);
    assert!(r.assumed.is_empty(), "{:?}", r.assumed);
    assert!(r.errors().is_empty(), "{:?}", r.errors());
}

#[test]
fn only_the_stated_modes_are_not_warned_about() {
    let r = run_power_on(
        &PowerOn {
            units: Some(LengthUnit::Metric),
            ..PowerOn::default()
        },
        PROGRAM,
    );
    assert!(
        r.assumed.iter().all(|d| !d.message.contains("G21")),
        "{:?}",
        r.assumed
    );
    assert!(r.assumed.iter().any(|d| d.message.contains("G90")
        || d.message.contains("G00")
        || d.message.contains("G17")));
}

#[test]
fn inches_at_power_on_read_lengths_in_inches() {
    let r = run_power_on(
        &PowerOn {
            units: Some(LengthUnit::Inch),
            ..stated()
        },
        "G0 X1.\n",
    );
    let x = target(r.moves()[0])[0].value;
    assert!((x - 25.4).abs() < 1e-9, "{x}");
}

#[test]
fn incremental_and_linear_at_power_on_are_used() {
    let p = PowerOn {
        distance: Some(DistanceMode::Incremental),
        motion: Some(StartMotion::Linear),
        ..stated()
    };
    let r = run_power_on(&p, "F100.\nX5.\nX5.\n");
    assert!(r.assumed.is_empty(), "{:?}", r.assumed);
    let moves = r.moves();
    assert!(
        matches!(moves[0], dialect_api::Command::Linear { .. }),
        "{moves:?}"
    );
    assert_eq!(moves.len(), 2);
}

#[test]
fn a_plane_fanuc_cannot_name_is_left_to_the_program() {
    // G17 is XY, G18 ZX, G19 YZ: X then Z is none of them.
    let r = run_power_on(
        &PowerOn {
            plane: Some(('X', 'Z')),
            ..stated()
        },
        PROGRAM,
    );
    assert!(
        r.assumed.iter().any(|d| d.message.contains("G17")),
        "{:?}",
        r.assumed
    );
}

#[test]
fn the_zx_plane_at_power_on_is_g18() {
    let r = run_power_on(
        &PowerOn {
            plane: Some(('Z', 'X')),
            ..stated()
        },
        "G2 Z10. X0. R5. F100.\n",
    );
    assert!(r.assumed.is_empty(), "{:?}", r.assumed);
    assert!(r.errors().is_empty(), "{:?}", r.errors());
}
