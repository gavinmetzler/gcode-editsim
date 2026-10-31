//! G92/G98 preset the axes until G99 cancels them (manual p. 2-82). This
//! is not simulated (coordinate shifts are on hold, owner decision
//! 2026-10-31), but moves are never hidden: one warning, the preset values
//! are data, and later moves are drawn without the preset. G99 must be
//! alone in its block (compatibility table, p. 1-10).

mod common;

use common::run;
use ir::{DiagCode, Severity};

#[test]
fn preset_is_ignored_with_a_warning() {
    let r = run("G71 G0 X100 Y80\nG92 X0 Y0\nG0 X10\nG99\nG0 X1 Y1 Z1\n");
    let d = r
        .diags
        .iter()
        .find(|d| d.code == DiagCode::UnsupportedGCode)
        .expect("a warning");
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.src.line, 2);
    assert!(!r.position_lost());
    // Every move is drawn; the preset itself is not a move.
    assert_eq!(r.moves().len(), 3);
}

#[test]
fn g99_must_be_alone() {
    let r = run("G99 X0\n");
    assert!(r.has(DiagCode::ModalGroupConflict, "G99 must be alone"));
}
