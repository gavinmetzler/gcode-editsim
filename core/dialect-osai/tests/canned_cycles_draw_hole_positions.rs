//! Canned cycles G81-G89 are not simulated (on hold, owner decision
//! 2026-10-31), but moves are never hidden: each hole is drawn as a rapid
//! to its position in the working plane, until G80. One warning per cycle
//! code.

mod common;

use common::run;
use ir::{DiagCode, Severity};

#[test]
fn holes_are_rapids_in_the_plane() {
    let r = run("G71 G17 F100\nG81 X10 Y10 Z-5 R2\nX20\nG80\nG0 X0 Y0 Z10\n");
    let d = r
        .diags
        .iter()
        .find(|d| d.code == DiagCode::UnsupportedGCode)
        .expect("a warning");
    assert_eq!(d.severity, Severity::Warning);
    assert!(d.message.contains("hole positions"), "{}", d.message);
    assert!(!r.position_lost());
    // Two holes (in X and Y only), then the move after G80.
    assert_eq!(r.moves().len(), 3, "{:?}", r.commands);
}
