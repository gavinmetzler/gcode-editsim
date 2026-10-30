//! Parentheses hold three-letter commands, not comments (manual p. 1-4);
//! comments start with ';'.

mod common;

use common::run;
use dialect_api::Command;
use ir::{DiagCode, WorkOffsetId};

#[test]
fn uao_selects_an_origin_for_all_axes() {
    let r = run("(UAO,1)\n(UAO, 2)\n(UAO,0)\n");
    let offsets: Vec<_> = r
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::SetWorkOffset(id) => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(
        offsets,
        vec![WorkOffsetId(1), WorkOffsetId(2), WorkOffsetId(0)]
    );
    assert!(r.diags.is_empty(), "{:?}", r.diags);
}

#[test]
fn uao_in_a_block_with_motion_applies_first() {
    let r = run("(UAO,1) G0 X5\n");
    assert!(matches!(
        r.commands[0],
        Command::SetWorkOffset(WorkOffsetId(1))
    ));
    assert_eq!(r.moves().len(), 1);
}

#[test]
fn per_axis_origins_hide_motion_until_one_origin_for_all() {
    let r = run("(UAO,2,X,Y)\nG0 X1\n(UAO,1)\nG0 X2 Y2 Z2\n");
    assert!(r.has(DiagCode::NotYetSimulated, "(UAO,2,X,Y) is not supported"));
    assert!(r.position_lost());
    // After (UAO,1) motion is emitted again.
    assert_eq!(r.moves().len(), 1);
}

#[test]
fn messages_and_graphics_commands_do_not_affect_the_path() {
    let r = run("(DIS,\"ROUGHING\")\n(UGS,1,X,-50,100,Y,50,100)\n(CGS)\nG71 G0 X1\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    assert_eq!(r.moves().len(), 1);
}

#[test]
fn other_commands_hide_motion_to_the_end() {
    let r = run("(ROT,45)\nG0 X1 Y1 Z1\nG0 X2 Y2 Z2\n");
    assert!(r.has(DiagCode::NotYetSimulated, "(ROT) is not supported"));
    assert!(r.moves().is_empty());
    assert!(r.position_lost());
}

#[test]
fn a_fanuc_style_comment_is_reported_not_obeyed() {
    let r = run("(TOOL 1 DRILL)\nG0 X1\n");
    assert!(r.has(DiagCode::UnsupportedWord, "not a three-letter command"));
    assert_eq!(r.moves().len(), 1);
}

#[test]
fn semicolon_comments_are_ignored() {
    let r = run(";THIS IS A COMMENT\nG1 X100 Y50 F100 ; motion block\n");
    assert!(r.diags.iter().all(|d| d.code != DiagCode::UnsupportedWord));
    assert_eq!(r.moves().len(), 1);
}
