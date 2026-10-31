//! A lower-case r rounds the corner to the next move (manual, U-axis
//! example p. 2-28). The parser reads it as R. Outside an arc it is not
//! simulated: a warning, and the move is drawn with a sharp corner (moves
//! are never hidden, owner decision 2026-10-31).

mod common;

use common::run;
use ir::{DiagCode, Severity};

#[test]
fn r_outside_an_arc_draws_a_sharp_corner() {
    let r = run("G71 G1 X10 r5 F100\n");
    let d = r
        .diags
        .iter()
        .find(|d| d.code == DiagCode::UnsupportedWord)
        .expect("a diagnostic");
    assert_eq!(d.severity, Severity::Warning);
    assert!(d.message.contains("R outside an arc"), "{}", d.message);
    assert_eq!(r.moves().len(), 1);
    assert!(!r.position_lost());
}

#[test]
fn r_alone_moves_nothing() {
    let r = run("N130 r5\n");
    assert!(r.has(DiagCode::UnsupportedWord, "R outside an arc"));
    assert!(r.moves().is_empty());
    assert!(!r.position_lost());
}
