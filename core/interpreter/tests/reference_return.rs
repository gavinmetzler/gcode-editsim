//! G28 (reference return) and G53 (machine coordinates) move the machine;
//! the simulated position must follow, not stay where it was.
//!
//! Regression test for review finding C2.

mod common;

use common::{at, codes, moves, run};
use ir::{DiagCode, SegKind};

#[test]
fn g91_g28_z0_retracts_z_to_the_reference_position() {
    // The generic mill's reference position is machine X0 Y0 Z0.
    let p = run("G21 G90 G54 G0 X10. Y10. Z5.\nG1 Z-10. F100.\nG91 G28 Z0.\nG90\n");
    let m = moves(&p);
    let last = m.last().unwrap();
    assert_eq!(last.kind, SegKind::Rapid);
    at(&last.end, [-370.0, -195.0, 0.0]);
    assert!(
        !codes(&p).contains(&DiagCode::UnsupportedGCode),
        "{:?}",
        p.diagnostics
    );
}

#[test]
fn next_tool_approach_starts_from_the_reference_position() {
    let src = "G21 G90 G54 G0 X10. Y10. Z5.\nG1 Z-10. F100.\nG91 G28 Z0.\nG28 X0. Y0.\nG90\nT2 M6\nG0 X50. Y0.\n";
    let p = run(src);
    let last = *moves(&p).last().unwrap();
    // Approach rapid starts at the reference position, not at Z-10 in the part.
    at(&last.start, [0.0, 0.0, 0.0]);
    at(&last.end, [-330.0, -205.0, 0.0]);
}

#[test]
fn g28_in_absolute_mode_goes_via_the_intermediate_point() {
    let p = run("G21 G90 G54 G0 X10. Y10. Z5.\nG28 Z50.\n");
    let m = moves(&p);
    let n = m.len();
    // Intermediate point at part Z50 (machine -350), then the reference (0).
    at(&m[n - 2].end, [-370.0, -195.0, -350.0]);
    at(&m[n - 1].end, [-370.0, -195.0, 0.0]);
}

#[test]
fn g53_moves_in_machine_coordinates() {
    let p = run("G21 G90 G54 G0 X10. Y10. Z5.\nG53 Z-100.\nG0 X20.\n");
    let m = moves(&p);
    at(&m[1].end, [-370.0, -195.0, -100.0]);
    // Following moves are back in the work offset, starting from there.
    at(&m[2].start, [-370.0, -195.0, -100.0]);
    at(&m[2].end, [-360.0, -195.0, -100.0]);
}
