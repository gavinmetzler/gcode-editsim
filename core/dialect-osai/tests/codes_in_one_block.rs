//! Two codes of one group in a block are incompatible (manual p. 1-10):
//! the control reports an error, so the block is skipped.

mod common;

use common::run;
use ir::DiagCode;

#[test]
fn same_group_codes_are_an_error() {
    for src in ["G0 G1 X10\n", "G90 G91 X1\n", "G70 G71 X1\n"] {
        let r = run(src);
        assert!(
            r.has(DiagCode::ModalGroupConflict, "cannot be in the same block"),
            "{src}"
        );
        assert!(r.commands.is_empty(), "{src}");
    }
}

#[test]
fn unknown_codes_are_reported() {
    let r = run("G33 Z-100 K2\nM60\n");
    assert!(r.has(DiagCode::UnsupportedGCode, "G33"));
    assert!(r.has(DiagCode::UnsupportedMCode, "M60"));
}
