//! A group-01 code (G00-G03) cancels a canned cycle, like G80.
//!
//! Regression test for review finding I1.

mod common;

use common::{run, w};
use dialect_api::Command;
use ir::DiagCode;

#[test]
fn g0_cancels_the_canned_cycle_and_moves() {
    let r = run("G81 X10. Y10. Z-5. R2. F100.\nG0 Z50.\nX100.\n");
    assert_eq!(
        r.moves(),
        vec![
            &Command::Rapid {
                target: vec![w(2, 50.0)]
            },
            &Command::Rapid {
                target: vec![w(0, 100.0)]
            },
        ]
    );
    let lines: Vec<(u32, DiagCode)> = r.errors().iter().map(|d| (d.src.line, d.code)).collect();
    assert_eq!(lines, vec![(1, DiagCode::UnsupportedGCode)]);
}

#[test]
fn g1_g2_g3_also_cancel() {
    for code in ["G1 X5. F100.", "G2 X5. R5. F100.", "G3 X5. R5. F100."] {
        let r = run(&format!("G81 X1. Y1. Z-5. R2. F100.\n{code}\n"));
        assert_eq!(r.moves().len(), 1, "{code}: {:?}", r.diags);
    }
}
