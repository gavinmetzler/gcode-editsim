//! M codes Fanuc does not define are the machine builder's (head selection,
//! clamps, vacuum, ...). They are noted once per code as machine-specific
//! and ignored, not reported as errors: they do not move the axes on the
//! machines seen so far, and motion continues. A code that does move a
//! machine can be listed in the machine file's `ignore_codes`, or will draw
//! differently from that machine; the note says so.

mod common;

use common::{run, w};
use dialect_api::Command;
use ir::{DiagCode, Severity};

#[test]
fn motion_continues() {
    let r = run("M92\nG0 X1. M213\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 1.0)]
        }]
    );
    assert!(r.errors().is_empty(), "{:?}", r.errors());
}

#[test]
fn each_code_is_noted_once() {
    let r = run("M92\nM213\nM92\nM92\nM213\n");
    let notes: Vec<_> = r
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::UnsupportedMCode)
        .collect();
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert!(notes.iter().all(|d| d.severity == Severity::Info));
    assert_eq!((notes[0].src.line, notes[1].src.line), (1, 2));
    assert!(notes[0].message.contains("M92"), "{}", notes[0].message);
    assert!(
        notes[0].message.contains("machine-specific"),
        "{}",
        notes[0].message
    );
}
