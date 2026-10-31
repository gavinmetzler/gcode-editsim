//! Codes outside the supported subset are reported, never silently ignored.

mod common;

use common::run;
use dialect_api::Command;
use ir::{DiagCode, Severity};

#[test]
fn unknown_g_code_is_an_error_and_the_block_does_not_move() {
    let r = run("G5 X10.\n");
    assert_eq!(r.codes(), vec![DiagCode::UnsupportedGCode]);
    assert_eq!(r.diags[0].severity, Severity::Error);
    assert!(r.moves().is_empty());
    // Its words are data; nothing is hidden and the position stays known.
    assert_eq!(r.commands, vec![Command::Unsupported { what: "G5".into() }]);
}

#[test]
fn one_shot_codes_use_axis_words_as_data() {
    // G4 X is a dwell time and G92/G10/G52 X set coordinates: none is a
    // move. (G28 and G53 are supported; see tests/reference_and_machine_moves.rs.)
    for src in ["G4 X1.\n", "G92 X0\n", "G10 L2 P1 X-100.\n", "G52 X5.\n"] {
        let r = run(src);
        assert!(r.moves().is_empty(), "{src}");
        assert!(
            !r.commands
                .iter()
                .any(|c| matches!(c, Command::PositionUnknown { .. })),
            "{src}: data codes do not move the machine"
        );
        assert_eq!(r.codes(), vec![DiagCode::UnsupportedGCode], "{src}");
    }
}

#[test]
fn cutter_compensation_warns_and_still_moves() {
    let r = run("G41 D1 G1 X5. F100.\n");
    let found: Vec<(DiagCode, Severity)> = r.diags.iter().map(|d| (d.code, d.severity)).collect();
    assert_eq!(found, vec![(DiagCode::NotYetSimulated, Severity::Warning)]);
    assert_eq!(r.moves().len(), 1);
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
