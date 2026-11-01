//! The simple time estimate (M3.8): each segment's time at its feed or
//! rapid rate, without acceleration; dwells and tool changes add their
//! time. The generic mill rapids X and Y at 30 000 mm/min and Z at
//! 24 000, and feeds at most 10 000 mm/min.

mod common;

use std::f64::consts::PI;

use common::{moves, segments};
use dialect_fanuc::Fanuc;
use interpreter::{interpret, Options};
use ir::{DiagCode, Program, SegKind, Severity};
use machine_mill::Mill;

fn close(a: Option<f64>, b: f64) {
    assert!(a.is_some_and(|a| (a - b).abs() < 1e-9), "{a:?} vs {b}");
}

/// Runs on the generic mill with `extra` added to its `[machine]` table.
fn run_on(extra: &str, src: &str) -> Program {
    let text = machine_mill::EXAMPLE_DEFINITION.replacen(
        "[machine]\n",
        &format!("[machine]\n{extra}\n"),
        1,
    );
    let mill = Mill::from_toml_str(&text).expect("machine loads");
    let fanuc = Fanuc::new().expect("dialect loads");
    interpret(src.as_bytes(), "test.nc", &fanuc, &mill, Options::default())
}

const START: &str = "G21 G90 G17 G54 G0 X0. Y0. Z0.\n";

fn run(rest: &str) -> Program {
    common::run(&format!("{START}{rest}"))
}

#[test]
fn a_feed_move_takes_its_length_over_the_feed_rate() {
    let p = run("G1 X100. F500.\n");
    // 100 mm at 500 mm/min: 12 s.
    close(moves(&p)[1].time, 12.0);
}

#[test]
fn a_rapid_takes_as_long_as_its_slowest_axis() {
    let p = run("");
    // From machine zero to G54 zero (X-380 Y-205 Z-400): X 0.76 s,
    // Y 0.41 s, Z 400 mm at 24 000 mm/min = 1 s.
    close(moves(&p)[0].time, 1.0);
}

#[test]
fn an_arc_is_timed_along_the_arc() {
    let p = run("G1 X10. F600.\nG3 X0. Y10. R10.\n");
    // A quarter circle of radius 10 (5π mm) at 600 mm/min.
    close(moves(&p)[2].time, 5.0 * PI / 600.0 * 60.0);
}

#[test]
fn a_feed_above_the_machine_maximum_runs_at_the_maximum() {
    let p = run("G1 X100. F20000.\n");
    close(moves(&p)[1].time, 0.6);
}

#[test]
fn feed_per_revolution_uses_the_spindle_speed() {
    let p = run("S1000 M3\nG95 G1 X10. F.1\n");
    let m = moves(&p)[1];
    // 0.1 mm/rev at 1000 rpm: 100 mm/min, so 10 mm takes 6 s.
    assert!((m.feed - 100.0).abs() < 1e-9, "{}", m.feed);
    close(m.time, 6.0);
}

#[test]
fn feed_per_revolution_with_the_spindle_stopped_has_no_time() {
    let p = run("G95 G1 X10. F.1\nX20.\n");
    let m = moves(&p);
    assert_eq!((m[1].time, m[2].time), (None, None));
    let warnings: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagCode::MissingFeedRate)
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].severity, Severity::Warning);
    assert_eq!(warnings[0].src.line, 2);
}

#[test]
fn events_take_no_time_and_tool_changes_take_the_machines_time() {
    let p = run("T1 M6\nM3 S1000\nM5\n");
    for s in segments(&p).iter().filter(|s| !s.kind.is_motion()) {
        close(s.time, 0.0);
    }
    let p = run_on("tool_change_time = 7.5", &format!("{START}T1 M6\nT2 M6\n"));
    let changes: Vec<_> = segments(&p)
        .iter()
        .filter(|s| s.kind == SegKind::ToolChange)
        .collect();
    assert_eq!(changes.len(), 2);
    for c in changes {
        close(c.time, 7.5);
    }
}

#[test]
fn a_rapid_on_an_axis_without_a_rapid_rate_has_no_time() {
    // Remove Z's rapid rate (the third `rapid =` line).
    let text = machine_mill::EXAMPLE_DEFINITION.replacen("rapid = 24000.0", "", 1);
    let mill = Mill::from_toml_str(&text).expect("machine loads");
    let fanuc = Fanuc::new().expect("dialect loads");
    let p = interpret(
        format!("{START}G0 X10.\nG0 Z10.\n").as_bytes(),
        "test.nc",
        &fanuc,
        &mill,
        Options::default(),
    );
    let m = moves(&p);
    // X still has a rate; Z does not.
    close(m[1].time, 10.0 / 30000.0 * 60.0);
    assert_eq!(m[2].time, None);
}

#[test]
fn the_program_total_adds_up_the_known_times() {
    let p = run("G1 X100. F500.\nG0 Z10.\n");
    let sum: f64 = segments(&p).iter().filter_map(|s| s.time).sum();
    close(p.meta.estimated_time, sum);
    assert!(sum > 13.0);
}

#[test]
fn feed_per_revolution_with_no_known_spindle_speed_says_so() {
    // The spindle turns (M3) but no S has been given: the speed is unknown,
    // not zero, so the warning must not say the spindle is stopped.
    let p = run("M3\nG95 G1 X10. F.1\n");
    let w = p
        .diagnostics
        .iter()
        .find(|d| d.code == DiagCode::MissingFeedRate)
        .expect("warned");
    assert!(w.message.contains("no spindle speed"), "{}", w.message);
    assert!(!w.message.contains("stopped"), "{}", w.message);
    assert_eq!(moves(&p)[1].time, None);
}

#[test]
fn feed_per_revolution_with_the_spindle_off_says_it_is_stopped() {
    let p = run("S1000\nG95 G1 X10. F.1\n");
    let w = p
        .diagnostics
        .iter()
        .find(|d| d.code == DiagCode::MissingFeedRate)
        .expect("warned");
    assert!(w.message.contains("spindle stopped"), "{}", w.message);
}
