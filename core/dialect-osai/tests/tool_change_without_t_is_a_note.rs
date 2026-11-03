//! M5.6d (owner, 2026-11-03): M6 with no T word is normal on some machines
//! (a head change), so it is a note, not an error.

mod common;

use common::run;
use ir::{DiagCode, Severity};

#[test]
fn m6_with_no_tool_selected_is_a_note() {
    let r = run("G71 G90 G17\nM6\nG0 X1\n");
    let found: Vec<_> = r
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::MissingWord)
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].severity, Severity::Info);
}
