//! M5.6d (owner, 2026-11-03): cutter compensation is not simulated (wishlist),
//! and a program that uses it gets a single warning, not one per block.

mod common;

use common::run;
use ir::{DiagCode, Severity};

#[test]
fn a_program_that_switches_compensation_on_many_times_gets_one_warning() {
    let r = run("G71 G90 G17\nG42 G1 X5 F100\nG40 X10\nG41 X15\nG40 X20\nG42 X25\n");
    let comp: Vec<_> = r
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::NotYetSimulated && d.message.contains("ompensation"))
        .collect();
    assert_eq!(comp.len(), 1, "{comp:?}");
    assert_eq!(comp[0].severity, Severity::Warning);
    assert_eq!(comp[0].src.line, 2);
}
