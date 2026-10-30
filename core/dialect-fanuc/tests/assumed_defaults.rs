//! Modes the program never sets are controller-dependent at power-on; the
//! assumed value is used with a warning, once per program.
//!
//! Regression test for review finding I5.

mod common;

use common::run;
use ir::{DiagCode, Severity};

fn assumed(src: &str) -> Vec<String> {
    let r = run(src);
    // Nothing that looks like an assumption may hide among the other
    // diagnostics.
    assert!(r.diags.iter().all(|d| !d.message.contains("assumed")));
    r.assumed
        .iter()
        .inspect(|d| {
            assert_eq!(d.code, DiagCode::ControllerDependent);
            assert_eq!(d.severity, Severity::Warning);
            assert!(d.dialect_specific);
        })
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn program_that_sets_everything_gets_no_warning() {
    assert!(assumed("G21 G17 G90 G0 X1.\nG2 X2. R1. F100.\n").is_empty());
}

#[test]
fn missing_units_is_warned_once() {
    let w = assumed("G17 G90 G0 X1.\nX2.\n");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("G21"), "{}", w[0]);
}

#[test]
fn missing_distance_mode_is_warned() {
    let w = assumed("G21 G17 G0 X1.\n");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("G90"), "{}", w[0]);
}

#[test]
fn missing_motion_mode_is_warned() {
    let w = assumed("G21 G90 X1.\n");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("G00"), "{}", w[0]);
}

#[test]
fn missing_plane_is_warned_on_the_first_arc_only() {
    assert!(assumed("G21 G90 G1 X1. F100.\n").is_empty());
    let w = assumed("G21 G90 G2 X1. R1. F100.\nG2 X2. R1.\n");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("G17"), "{}", w[0]);
}

#[test]
fn units_warning_also_covers_feed() {
    let w = assumed("G90 G0\nF100.\n");
    assert_eq!(w.len(), 1, "{w:?}");
}
