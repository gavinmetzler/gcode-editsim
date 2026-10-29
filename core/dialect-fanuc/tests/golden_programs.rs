//! The sample programs in tests/golden/ interpret as expected.

mod common;

use std::path::Path;

use common::run;
use ir::{DiagCode, Severity};

fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden")
        .join(name);
    std::fs::read_to_string(path).expect("golden file exists")
}

#[test]
fn basic_mill_program_has_no_errors() {
    let r = run(&golden("basic_mill.nc"));
    assert!(r.errors().is_empty(), "{:?}", r.errors());
    // G43 H1 (tool length compensation) is the only thing not simulated.
    let warnings: Vec<DiagCode> = r
        .diags
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .map(|d| d.code)
        .collect();
    assert_eq!(warnings, vec![DiagCode::NotYetSimulated]);
    assert_eq!(r.moves().len(), 8);
}

#[test]
fn error_program_reports_every_bad_line_and_keeps_going() {
    let r = run(&golden("errors_and_recovery.nc"));
    let lines: Vec<(u32, DiagCode)> = r.errors().iter().map(|d| (d.src.line, d.code)).collect();
    // Lines 2 and 4-8 have syntax errors. Lines 3 and 9 parse cleanly but are
    // G1 moves in a file that never sets a feed rate, which Fanuc rejects.
    assert_eq!(
        lines,
        vec![
            (2, DiagCode::SyntaxError),
            (3, DiagCode::MissingFeedRate),
            (4, DiagCode::SyntaxError),
            (5, DiagCode::SyntaxError),
            (6, DiagCode::SyntaxError),
            (7, DiagCode::SyntaxError),
            (8, DiagCode::SyntaxError),
            (9, DiagCode::MissingFeedRate),
        ]
    );
    assert!(r.moves().is_empty());
}
