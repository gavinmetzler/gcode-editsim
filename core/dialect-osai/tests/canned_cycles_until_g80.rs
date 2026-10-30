//! Canned cycles G81-G89 are not simulated; motion is hidden until G80.

mod common;

use common::run;
use ir::DiagCode;

#[test]
fn cycle_hides_motion_until_g80() {
    let r = run("G71 F100\nG81 X10 Y10 Z-5 R2\nX20\nG80\nG0 X0 Y0 Z10\n");
    assert!(r.has(DiagCode::UnsupportedGCode, "G81 (Drilling cycle)"));
    assert!(r.position_lost());
    assert_eq!(r.moves().len(), 1);
}
