//! A per-axis origin, `(UAO,n,axes...)` (manual p. 2-95), is not simulated
//! yet (planned for M3.5). At the owner's request (2026-10-31) it is
//! soft-ignored: a warning at the line, the current origin stays in use for
//! every axis, and moves keep being drawn. An origin number that is not
//! valid is still an error that hides later moves, because then nothing is
//! known about the origin in use.

mod common;

use common::run;
use dialect_api::Command;
use ir::{DiagCode, Severity, WorkOffsetId};

#[test]
fn moves_keep_being_drawn_with_the_current_origin() {
    let r = run("(UAO,1)\n(UAO,2,Z)\nG0 X1 Y1 Z1\n");
    assert_eq!(r.moves().len(), 1);
    assert!(!r.position_lost());
    let offsets: Vec<_> = r
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::SetWorkOffset(id) => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(offsets, vec![WorkOffsetId(1)], "origin 1 stays in use");
}

#[test]
fn it_is_a_warning_naming_the_axes() {
    let r = run("(UAO,2,X,Y)\nG0 X1\n");
    let d = r
        .diags
        .iter()
        .find(|d| d.code == DiagCode::NotYetSimulated)
        .expect("a diagnostic");
    assert_eq!(d.severity, Severity::Warning);
    assert!(d.message.contains("(UAO,2,X,Y)"), "{}", d.message);
    assert!(d.message.contains("X, Y"), "{}", d.message);
    assert!(r.errors().is_empty(), "{:?}", r.errors());
}

#[test]
fn spaces_after_commas_are_allowed() {
    let r = run("(UAO, 3, Z)\nG0 Z5\n");
    assert_eq!(r.moves().len(), 1);
    assert!(r.errors().is_empty(), "{:?}", r.errors());
}

#[test]
fn an_invalid_origin_number_still_hides_motion() {
    let r = run("(UAO,11,Z)\nG0 X1\n(UAO,1)\nG0 X2 Y2 Z2\n");
    assert!(r.has(DiagCode::NotYetSimulated, "(UAO,11,Z) is not supported"));
    assert!(r.position_lost());
    // After (UAO,1) motion is emitted again.
    assert_eq!(r.moves().len(), 1);
}
