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

#[test]
fn each_axis_is_named_once_with_its_furthest_point() {
    // From machine X+20 to X+30: both ends are past the X <= 0 limit. The
    // warning names X once, at the furthest point (30), not once per end.
    let p = run("G0 X400. Y0\nG0 X410.\n");
    let d = p
        .diagnostics
        .iter()
        .find(|d| d.src.line == 2)
        .expect("line 2 reported");
    assert_eq!(
        d.message,
        "move goes outside the travel limits: X reaches 30.000 (travel -760.000 to 0.000)"
    );
}

#[test]
fn a_move_past_both_ends_of_an_axis_names_the_axis_once() {
    // From machine X+20 to X-1000 crosses the whole X travel: past the top
    // at the start and past the bottom at the end. Named once, at the point
    // furthest outside (1000 - 760 = 240 below, versus 20 above).
    let p = run("G0 X400. Y0\nG0 X-620.\n");
    let d = p
        .diagnostics
        .iter()
        .find(|d| d.src.line == 2)
        .expect("line 2 reported");
    assert_eq!(d.message.matches("X reaches").count(), 1, "{}", d.message);
    assert!(d.message.contains("X reaches -1000.000"), "{}", d.message);
}
