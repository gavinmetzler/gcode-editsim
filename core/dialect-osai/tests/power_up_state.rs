//! The 10 Series power-up state (manual p. 1-11/1-12): G00, G17, G71, G90,
//! G94, and all axes referred to the home position (UAO, p. 2-95).

mod common;

use common::{abs, axes, run, xyz};
use dialect_api::{Command, Dialect, DistanceMode, FeedMode};
use dialect_osai::Osai;
use ir::{ArcPlane, DiagCode, LengthUnit, WorkOffsetId};

#[test]
fn initial_state_follows_the_power_up_column() {
    let osai = Osai::new().expect("loads");
    let s = osai.initial_state(&xyz());
    assert_eq!(s.units, LengthUnit::Metric);
    assert_eq!(s.distance, DistanceMode::Absolute);
    assert_eq!(s.feed_mode, FeedMode::PerMinute);
    assert_eq!(
        s.plane,
        Some(ArcPlane {
            first: 0,
            second: 1
        })
    );
    // Home position, not a work origin (unlike Fanuc's G54).
    assert_eq!(s.work_offset, WorkOffsetId(0));
    assert_eq!(s.active_code(1).map(|k| k.to_string()), Some("G0".into()));
}

#[test]
fn motion_mode_is_rapid_without_a_g_code() {
    let r = run("X10 Y20\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![abs(0, 10.0), abs(1, 20.0)]
        }]
    );
}

#[test]
fn numbers_without_a_decimal_point_are_whole_units() {
    // "X10 Y20" in the manual's examples is 10 mm and 20 mm.
    let r = run("G71 X10\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![abs(0, 10.0)]
        }]
    );
    assert!(!r.codes().contains(&DiagCode::ControllerDependent));
}

#[test]
fn relying_on_default_units_is_pointed_out_once() {
    let r = run("X10\nX20\nX30\n");
    let notes: Vec<_> = r
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::ControllerDependent && d.message.contains("G71"))
        .collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].src.line, 1);
}

#[test]
fn no_plane_on_a_machine_with_one_axis() {
    let osai = Osai::new().expect("loads");
    assert_eq!(osai.initial_state(&axes(&[('X', false)])).plane, None);
}
