//! Positions are machine coordinates with the active work offset applied.

mod common;

use common::{at, codes, moves, run};
use ir::{DiagCode, WorkOffsetId};

#[test]
fn starts_at_machine_zero_in_g54() {
    let p = run("G0 X10. Y20.\n");
    let m = moves(&p);
    at(&m[0].start, [0.0, 0.0, 0.0]);
    // G54 origin X-380 Y-205; Z is not programmed so stays at 0.
    at(&m[0].end, [-370.0, -185.0, 0.0]);
    assert_eq!(m[0].work_offset, WorkOffsetId(1));
}

#[test]
fn switching_work_offsets() {
    let p = run("G0 X0 Y0\nG55 X0 Y0\n");
    let m = moves(&p);
    at(&m[0].end, [-380.0, -205.0, 0.0]);
    at(&m[1].end, [-200.0, -205.0, 0.0]);
    assert_eq!(m[1].work_offset, WorkOffsetId(2));
}

#[test]
fn incremental_moves_ignore_the_offset() {
    let p = run("G0 X0 Y0\nG91 X5. Y-5.\nX5.\n");
    let m = moves(&p);
    at(&m[1].end, [-375.0, -210.0, 0.0]);
    at(&m[2].end, [-370.0, -210.0, 0.0]);
}

#[test]
fn undefined_work_offset_is_reported_once_and_machine_coordinates_used() {
    // G56 is not in the generic machine file. Negative values keep the moves
    // inside the travel limits in machine coordinates.
    let p = run("G56 G0 X-1.\nX-2.\n");
    assert_eq!(codes(&p), vec![DiagCode::InvalidValue]);
    assert_eq!(p.diagnostics[0].src.line, 1);
    at(&moves(&p)[1].end, [-2.0, 0.0, 0.0]);
}

#[test]
fn inch_values_are_already_mm() {
    let p = run("G20 G0 X1. Y1.\n");
    at(&moves(&p)[0].end, [-380.0 + 25.4, -205.0 + 25.4, 0.0]);
}
