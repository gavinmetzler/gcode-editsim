//! Moves outside the machine's travel are flagged and reported.

mod common;

use common::{codes, moves, run};
use ir::{DiagCode, SegFlags, Severity};

#[test]
fn move_past_a_limit_is_flagged_and_reported() {
    // G54 X origin is -380 and X travel ends at 0, so X400. is 20 mm over.
    let p = run("G0 X0 Y0\nG0 X400.\n");
    let m = moves(&p);
    assert!(!m[0].flags.contains(SegFlags::OUT_OF_LIMITS));
    assert!(m[1].flags.contains(SegFlags::OUT_OF_LIMITS));
    assert_eq!(codes(&p), vec![DiagCode::OutOfLimits]);
    let d = &p.diagnostics[0];
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.src.line, 2);
    assert!(d.message.contains("X reaches 20.000"), "{}", d.message);
}

#[test]
fn arc_bulging_past_a_limit_is_caught() {
    // Both end points are at machine X-4, inside the X <= 0 limit, but the
    // clockwise half circle swings out to machine X+1.
    let p = run("G0 X376. Y0\nG2 X376. Y-10. R5. F100.\n");
    assert!(!moves(&p)[0].flags.contains(SegFlags::OUT_OF_LIMITS));
    let m = moves(&p);
    assert!(
        m[1].flags.contains(SegFlags::OUT_OF_LIMITS),
        "{:?}",
        p.diagnostics
    );
}

#[test]
fn reports_are_capped_but_every_segment_is_flagged() {
    let src: String = (0..60).map(|i| format!("G0 X{}.\n", 400 + i)).collect();
    let p = run(&src);
    assert!(moves(&p)
        .iter()
        .all(|s| s.flags.contains(SegFlags::OUT_OF_LIMITS)));
    let warnings = codes(&p)
        .iter()
        .filter(|c| **c == DiagCode::OutOfLimits)
        .count();
    // 50 individual warnings plus one summary note.
    assert_eq!(warnings, 51);
    assert!(p
        .diagnostics
        .last()
        .unwrap()
        .message
        .contains("10 more moves"));
}
