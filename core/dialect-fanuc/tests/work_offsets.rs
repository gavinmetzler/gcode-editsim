//! G54-G59 select work offsets 1-6.

mod common;

use common::run;
use dialect_api::Command;
use ir::{DiagCode, WorkOffsetId};

#[test]
fn g54_to_g59() {
    let r = run("G54\nG55\nG56\nG57\nG58\nG59\n");
    let ids: Vec<WorkOffsetId> = r
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::SetWorkOffset(id) => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(ids, (1..=6).map(WorkOffsetId).collect::<Vec<_>>());
    assert!(r.diags.is_empty());
}

#[test]
fn extended_offsets_are_not_supported_yet() {
    let r = run("G54.1 P1 G0 X1.\n");
    assert!(r.codes().contains(&DiagCode::UnsupportedGCode));
    assert!(r.moves().is_empty());
}
