//! Tool length compensation (G43, G44, H) is never simulated: the backplot
//! shows the path as programmed (owner decision, 2026-10-31). One note per
//! program says so; it is not a warning, since nothing is missing.

mod common;

use common::run;
use ir::{DiagCode, Severity};

#[test]
fn the_path_is_drawn_as_programmed() {
    let r = run("G91 G43 Z-40. H11\nG0 X1.\n");
    assert_eq!(r.moves().len(), 2, "{:?}", r.commands);
    assert!(r.errors().is_empty(), "{:?}", r.errors());
}

#[test]
fn one_note_per_program() {
    let r = run("G43 H11 Z25.\nG49\nG43 H12 Z25.\nG44 H1 Z5.\n");
    let notes: Vec<_> = r
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::NotYetSimulated)
        .collect();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].severity, Severity::Info);
    assert_eq!(notes[0].src.line, 1);
    assert!(
        notes[0].message.contains("as programmed"),
        "{}",
        notes[0].message
    );
}
