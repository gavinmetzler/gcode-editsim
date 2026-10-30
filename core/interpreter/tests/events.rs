//! Stops, spindle, coolant and program end become zero-length event
//! segments in program order.

mod common;

use common::{kinds, run, segments};
use ir::{SegFlags, SegKind};

#[test]
fn each_event_gives_one_segment() {
    let p = run("M3 S1000\nM4\nM5\nM8\nM9\nM0\nM1\nM30\n");
    assert_eq!(
        kinds(&p),
        vec![
            SegKind::SpindleCw,
            SegKind::SpindleCcw,
            SegKind::SpindleStop,
            SegKind::CoolantFlood,
            SegKind::CoolantOff,
            SegKind::Stop,
            SegKind::OptionalStop,
            SegKind::ProgramEnd,
        ]
    );
    let lines: Vec<u32> = segments(&p).iter().map(|s| s.src.line).collect();
    assert_eq!(lines, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn m2_is_a_program_end_too() {
    let p = run("M2\n");
    assert_eq!(kinds(&p), vec![SegKind::ProgramEnd]);
}

#[test]
fn events_do_not_move_and_sit_at_the_current_position() {
    let p = run("G90 G0 X-100. Y-50. Z-10.\nM3 S500\nM8\nM0\n");
    let segs = segments(&p);
    let after_move = segs[0].end;
    for s in &segs[1..] {
        assert_eq!(s.start, after_move, "{:?}", s.kind);
        assert_eq!(s.end, after_move, "{:?}", s.kind);
        assert!(s.arc.is_none());
        assert_eq!(s.feed, 0.0);
        assert!(!s.kind.is_motion());
    }
}

#[test]
fn events_are_not_in_the_bounding_box() {
    let only_events = run("M3 S500\nM8\nM9\nM5\nM30\n");
    assert!(only_events.meta.bbox.is_none());
}

#[test]
fn events_keep_program_order_with_motion_in_the_same_block() {
    // Fanuc order within a block: M3/M8 before the move, M5/M9 and stops
    // after it (dialect-fanuc applies this; here we check it reaches the IR).
    let p = run("G0 X-1. M3 S800\nG0 X-2. M5\nG1 X-3. F100. M8\nG1 X-4. M9 \nG0 X-5. M0\n");
    assert_eq!(
        kinds(&p),
        vec![
            SegKind::SpindleCw,
            SegKind::Rapid,
            SegKind::Rapid,
            SegKind::SpindleStop,
            SegKind::CoolantFlood,
            SegKind::Linear,
            SegKind::Linear,
            SegKind::CoolantOff,
            SegKind::Rapid,
            SegKind::Stop,
        ]
    );
}

#[test]
fn spindle_event_carries_the_new_spindle_state() {
    let p = run("S1200 M3\nM5\n");
    let segs = segments(&p);
    assert_eq!(segs[0].kind, SegKind::SpindleCw);
    assert_eq!(segs[0].spindle_rpm, 1200.0);
    assert_eq!(segs[1].kind, SegKind::SpindleStop);
    assert_eq!(segs[1].spindle_rpm, 0.0);
}

#[test]
fn repeated_commands_are_each_recorded() {
    // Every command the program gives is shown at its line, even if it does
    // not change the state.
    let p = run("M5\nM9\nM9\n");
    assert_eq!(
        kinds(&p),
        vec![
            SegKind::SpindleStop,
            SegKind::CoolantOff,
            SegKind::CoolantOff
        ]
    );
}

#[test]
fn program_end_segment_is_last_and_later_blocks_add_nothing() {
    let p = run("G0 X-1.\nM30\nM3 S100\nG0 X-2.\n");
    assert_eq!(kinds(&p), vec![SegKind::Rapid, SegKind::ProgramEnd]);
}

#[test]
fn events_while_the_position_is_unknown_are_flagged() {
    // A move under G68 (not simulated) leaves the position unknown. The
    // coolant event still happened, but where is not known. G53 on every
    // axis makes the position known again.
    let src = "G0 X-1. Y-1. Z-1.\nG68 X0. Y0. R90.\nG0 X-2.\nM8\nG69\nG53 X-5. Y-5. Z-5.\nM9\n";
    let p = run(src);
    let events: Vec<_> = segments(&p)
        .iter()
        .filter(|s| matches!(s.kind, SegKind::CoolantFlood | SegKind::CoolantOff))
        .collect();
    assert_eq!(events.len(), 2);
    assert!(events[0].flags.contains(SegFlags::POSITION_UNKNOWN));
    assert!(!events[1].flags.contains(SegFlags::POSITION_UNKNOWN));
}

#[test]
fn motion_is_never_flagged_position_unknown() {
    let p = run("G0 X-1. Y-1. Z-1.\nM3 S100\nG1 X-2. F10.\n");
    for s in segments(&p) {
        assert!(!s.flags.contains(SegFlags::POSITION_UNKNOWN));
    }
}
