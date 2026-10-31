//! G92 (workpiece coordinate system setting) is not simulated yet; proper
//! support is planned for M3.5. At the owner's request (2026-10-31) it is
//! ignored with a warning instead of hiding every later move, because real
//! programs often start with it (`G54G92X1000.0Y1250.0Z0`). Its axis words
//! are data, never a move.

mod common;

use common::{run, w};
use dialect_api::Command;
use ir::{DiagCode, Severity};

#[test]
fn later_moves_are_drawn() {
    let r = run("G0 X1.\nG54 G92 X1000. Y1250. Z0\nG90 G0 X2.\nG1 X3. F100.\n");
    assert_eq!(r.moves().len(), 3, "{:?}", r.commands);
    assert!(r.errors().is_empty(), "{:?}", r.errors());
}

#[test]
fn its_axis_words_are_not_a_move() {
    let r = run("G0 X1.\nG92 X1000. Y1250. Z0\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 1.0)]
        }]
    );
    assert!(!r
        .commands
        .iter()
        .any(|c| matches!(c, Command::PositionUnknown { .. })));
}

#[test]
fn it_is_a_warning_at_the_line() {
    let r = run("G0 X1.\nG92 X0 Y0\n");
    let d = r
        .diags
        .iter()
        .find(|d| d.code == DiagCode::UnsupportedGCode)
        .expect("a diagnostic for G92");
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.src.line, 2);
    assert!(d.message.contains("G92"), "{}", d.message);
    assert!(d.message.contains("without"), "{}", d.message);
}
