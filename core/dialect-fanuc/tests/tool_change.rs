//! T selects a tool; M6 changes to the selected tool.

mod common;

use common::run;
use dialect_api::Command;
use ir::DiagCode;

#[test]
fn select_then_change_in_one_block() {
    let r = run("T5 M6\n");
    assert!(r.diags.is_empty());
    assert_eq!(
        r.commands,
        vec![Command::SelectTool(5), Command::ToolChange { tool: 5 }]
    );
}

#[test]
fn preselect_then_change_later() {
    let r = run("T7\nG0 X1.\nM6\n");
    assert_eq!(r.commands.last(), Some(&Command::ToolChange { tool: 7 }));
}

#[test]
fn change_without_selection_is_a_warning() {
    // Some machines use M6 without a T word for their own purposes (a head
    // change on a router, for example), so it is a warning, not an error.
    let r = run("M6\n");
    assert_eq!(r.codes(), vec![DiagCode::MissingWord]);
    assert!(r.errors().is_empty(), "{:?}", r.diags);
    assert!(r.commands.is_empty());
}

#[test]
fn tool_number_must_be_whole() {
    for src in ["T1.\n", "T-2\n"] {
        let r = run(src);
        assert_eq!(r.codes(), vec![DiagCode::InvalidValue], "{src}");
    }
}
