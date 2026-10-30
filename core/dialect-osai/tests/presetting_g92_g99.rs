//! G92/G98 preset the axes until G99 cancels them (manual p. 2-82); this
//! is not simulated, so motion is hidden in between. G99 must be alone in
//! its block (compatibility table, p. 1-10).

mod common;

use common::run;
use ir::DiagCode;

#[test]
fn preset_hides_motion_until_g99() {
    let r = run("G71 G0 X100 Y80\nG92 X0 Y0\nG0 X10\nG99\nG0 X1 Y1 Z1\n");
    assert!(r.has(DiagCode::UnsupportedGCode, "until G99"));
    assert!(r.position_lost());
    // The first move and the one after G99; not the preset or X10.
    assert_eq!(r.moves().len(), 2);
}

#[test]
fn g99_must_be_alone() {
    let r = run("G99 X0\n");
    assert!(r.has(DiagCode::ModalGroupConflict, "G99 must be alone"));
}
