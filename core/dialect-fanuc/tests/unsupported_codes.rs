//! Codes outside the supported subset are reported, never silently ignored.

mod common;

use common::{run, w};
use dialect_api::Command;
use ir::{DiagCode, Severity};

#[test]
fn unknown_g_code_is_an_error_and_the_block_does_not_move() {
    let r = run("G5 X10.\n");
    assert_eq!(r.codes(), vec![DiagCode::UnsupportedGCode]);
    assert_eq!(r.diags[0].severity, Severity::Error);
    assert!(r.moves().is_empty());
    assert_eq!(r.commands, vec![Command::Unsupported { what: "G5".into() }]);
}

#[test]
fn unknown_m_code_is_an_error_but_motion_continues() {
    let r = run("G0 X1. M50\n");
    assert_eq!(r.codes(), vec![DiagCode::UnsupportedMCode]);
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 1.0)]
        }]
    );
}

#[test]
fn canned_cycle_blocks_motion_until_cancelled() {
    let r = run("G81 X1. Y1. Z-5. R2. F100.\nX2.\nG80\nG0 X3.\n");
    let errors: Vec<(u32, DiagCode)> = r.errors().iter().map(|d| (d.src.line, d.code)).collect();
    assert_eq!(
        errors,
        vec![
            (1, DiagCode::UnsupportedGCode),
            (2, DiagCode::UnsupportedGCode)
        ]
    );
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 3.0)]
        }]
    );
}

#[test]
fn one_shot_codes_block_motion() {
    // G4 X is a dwell time and G28 X/Y is an intermediate point, not moves.
    for src in ["G4 X1.\n", "G28 X0 Y0\n", "G53 Z0\n", "G92 X0\n"] {
        let r = run(src);
        assert!(r.moves().is_empty(), "{src}");
        assert_eq!(r.codes(), vec![DiagCode::UnsupportedGCode], "{src}");
    }
}

#[test]
fn compensation_codes_warn_and_still_move() {
    let r = run("G43 H1 Z25.\nG41 D1 G1 X5. F100.\n");
    let found: Vec<(DiagCode, Severity)> = r.diags.iter().map(|d| (d.code, d.severity)).collect();
    assert_eq!(
        found,
        vec![
            (DiagCode::NotYetSimulated, Severity::Warning),
            (DiagCode::NotYetSimulated, Severity::Warning)
        ]
    );
    assert_eq!(r.moves().len(), 2);
}

#[test]
fn safety_line_cancel_codes_are_quiet() {
    let r = run("G17 G21 G40 G49 G80 G90 G94\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
}

#[test]
fn feed_per_revolution_is_reported_but_path_is_kept() {
    let r = run("G95 G1 X1. F.1\n");
    assert_eq!(r.codes(), vec![DiagCode::UnsupportedGCode]);
    assert_eq!(r.moves().len(), 1);
}

#[test]
fn subprogram_codes_are_reported() {
    let r = run("M98 P100\nM99\n");
    assert_eq!(
        r.codes(),
        vec![DiagCode::UnsupportedMCode, DiagCode::UnsupportedMCode]
    );
}
