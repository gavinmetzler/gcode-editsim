//! M5.6b: a machine file that states the control's power-on state means
//! the program may rely on it: the 10 Series "assumed G71 / G17" warnings
//! (the machine builder can change those defaults) are not given.

mod common;

use common::{run, run_power_on};
use dialect_api::{DistanceMode, PowerOn, StartMotion};
use ir::LengthUnit;

const PROGRAM: &str = "G0 X10 Y10\nG2 X20 Y10 I15 J10 F100\n";

fn assumed(r: &common::Run) -> Vec<String> {
    r.diags
        .iter()
        .filter(|d| d.message.starts_with("assumed "))
        .map(|d| d.message.clone())
        .collect()
}

fn stated() -> PowerOn {
    PowerOn {
        units: Some(LengthUnit::Metric),
        plane: Some(('X', 'Y')),
        motion: Some(StartMotion::Rapid),
        distance: Some(DistanceMode::Absolute),
    }
}

#[test]
fn without_a_stated_power_on_state_the_power_up_defaults_are_warned_about() {
    let a = assumed(&run(PROGRAM));
    assert!(a.iter().any(|m| m.contains("G71")), "{a:?}");
    assert!(a.iter().any(|m| m.contains("G17")), "{a:?}");
}

#[test]
fn a_stated_power_on_state_gives_no_assumed_warnings() {
    let r = run_power_on(PROGRAM, &stated());
    assert!(assumed(&r).is_empty(), "{:?}", assumed(&r));
}

#[test]
fn only_the_stated_units_are_not_warned_about() {
    let r = run_power_on(
        PROGRAM,
        &PowerOn {
            units: Some(LengthUnit::Metric),
            ..PowerOn::default()
        },
    );
    let a = assumed(&r);
    assert!(a.iter().all(|m| !m.contains("G71")), "{a:?}");
    assert!(a.iter().any(|m| m.contains("G17")), "{a:?}");
}

#[test]
fn inches_at_power_on_read_lengths_in_inches() {
    let r = run_power_on(
        "G0 X1\n",
        &PowerOn {
            units: Some(LengthUnit::Inch),
            ..stated()
        },
    );
    let dialect_api::Command::Rapid { target, .. } = r.moves()[0] else {
        panic!("a rapid: {:?}", r.moves());
    };
    let x = target[0].value;
    assert!((x - 25.4).abs() < 1e-9, "{x}");
}

#[test]
fn a_plane_the_10_series_cannot_name_is_left_to_the_program() {
    // G17 is axes 1-2, G18 axes 3-1, G19 axes 2-3: X then Z is none of them.
    let r = run_power_on(
        PROGRAM,
        &PowerOn {
            plane: Some(('X', 'Z')),
            ..stated()
        },
    );
    assert!(
        assumed(&r).iter().any(|m| m.contains("G17")),
        "{:?}",
        assumed(&r)
    );
}
