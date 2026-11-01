//! Positions in program coordinates (M4.5): for each run of each block in
//! a range of lines, the modal state, the commands, and the full position
//! before and after, in the block's work coordinates, from
//! `interpret_traced`. Normal runs build no trace.

mod common;

use dialect_api::{Command, DistanceMode};
use dialect_fanuc::Fanuc;
use interpreter::{interpret_traced, BlockTrace, Options};
use ir::{LengthUnit, Pos, WorkOffsetId};
use machine_mill::Mill;

/// Runs `src` on the generic mill (G54 at machine X-380 Y-205 Z-400, G55
/// at X-200), tracing lines `first..=last`.
fn trace(src: &str, first: u32, last: u32) -> Vec<BlockTrace> {
    let fanuc = Fanuc::new().expect("dialect");
    let mill = Mill::generic().expect("machine");
    interpret_traced(
        src.as_bytes(),
        "t.nc",
        &fanuc,
        &mill,
        Options::default(),
        first..=last,
    )
    .1
}

fn xyz(p: &Pos) -> [f64; 3] {
    [p[0], p[1], p[2]]
}

fn close(a: [f64; 3], b: [f64; 3]) {
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < 1e-9, "{a:?} != {b:?}");
    }
}

#[test]
fn absolute_moves_in_work_coordinates() {
    let t = trace(
        "G21 G90 G17 G54 G0 X0. Y0. Z5.\nG1 X10. Y20. F100.\nG0 Z50.\n",
        2,
        2,
    );
    assert_eq!(t.len(), 1);
    let b = &t[0];
    assert_eq!((b.line, b.block, b.run), (2, 1, 0));
    close(xyz(&b.before), [0.0, 0.0, 5.0]);
    close(xyz(&b.after), [10.0, 20.0, 5.0]);
    assert!(b.known_before && b.known_after);
    assert_eq!(b.state.distance, DistanceMode::Absolute);
    assert_eq!(b.state.units, LengthUnit::Metric);
    assert_eq!(b.state.work_offset, WorkOffsetId(1));
    assert!(matches!(
        b.commands.as_slice(),
        [.., Command::Linear { .. }]
    ));
    assert!(!b.errors);
}

#[test]
fn incremental_moves_give_the_full_position() {
    let t = trace(
        "G21 G90 G54 G0 X5. Y5. Z5.\nG91 G1 X10. F100.\nX10.\n",
        2,
        3,
    );
    assert_eq!(t.len(), 2);
    assert_eq!(
        t[0].state.distance,
        DistanceMode::Incremental,
        "G91 applies to its own block"
    );
    close(xyz(&t[0].before), [5.0, 5.0, 5.0]);
    close(xyz(&t[0].after), [15.0, 5.0, 5.0]);
    close(xyz(&t[1].before), [15.0, 5.0, 5.0]);
    close(xyz(&t[1].after), [25.0, 5.0, 5.0]);
}

#[test]
fn positions_are_in_mm_with_the_program_units_alongside() {
    let t = trace("G21 G90 G54 G0 X0. Y0. Z5.\nG20 G0 X1.\nG21 G0 X2.\n", 2, 3);
    assert_eq!(
        t[0].state.units,
        LengthUnit::Inch,
        "G20 applies to its own block"
    );
    close(xyz(&t[0].after), [25.4, 0.0, 5.0]);
    assert_eq!(t[1].state.units, LengthUnit::Metric);
    close(xyz(&t[1].before), [25.4, 0.0, 5.0]);
    close(xyz(&t[1].after), [2.0, 0.0, 5.0]);
}

#[test]
fn both_positions_are_in_the_work_offset_of_the_block() {
    // G55 is 180 mm further along X than G54.
    let t = trace("G21 G90 G54 G0 X0. Y0. Z5.\nG55 G0 X10.\nG0 X20.\n", 2, 3);
    assert_eq!(t[0].state.work_offset, WorkOffsetId(2));
    close(xyz(&t[0].before), [-180.0, 0.0, 5.0]);
    close(xyz(&t[0].after), [10.0, 0.0, 5.0]);
    close(xyz(&t[1].before), [10.0, 0.0, 5.0]);
}

#[test]
fn every_run_of_a_subprogram_block_is_traced() {
    let src = "G21 G90 G17 G54 G0 X0. Y0. Z5.\n\
               M98 P10 L2\n\
               G0 X50.\n\
               M30\n\
               O10\n\
               G91 G0 X10.\n\
               G90\n\
               M99\n";
    let t = trace(src, 6, 6);
    assert_eq!(t.len(), 2);
    assert_eq!((t[0].run, t[1].run), (0, 1));
    close(xyz(&t[0].before), [0.0, 0.0, 5.0]);
    close(xyz(&t[0].after), [10.0, 0.0, 5.0]);
    close(xyz(&t[1].after), [20.0, 0.0, 5.0]);
    // In run order, between other blocks of the range.
    let t = trace(src, 2, 7);
    let lines: Vec<u32> = t.iter().map(|b| b.line).collect();
    assert_eq!(lines, vec![2, 6, 7, 6, 7, 3, 4]);
}

#[test]
fn only_the_asked_lines_are_traced() {
    let src = "G21 G90 G54 G0 X0. Y0. Z5.\nG1 X1. F100.\nX2.\nX3.\nM30\n";
    let lines = |t: Vec<BlockTrace>| t.iter().map(|b| b.line).collect::<Vec<_>>();
    assert_eq!(lines(trace(src, 3, 4)), vec![3, 4]);
    assert_eq!(
        lines(trace(src, 4, 3)),
        vec![3, 4],
        "a range given backwards"
    );
    assert!(
        trace(src, 7, 9).is_empty(),
        "lines the program does not have"
    );
    // Blank lines and comment-only lines have no block or do nothing.
    assert_eq!(lines(trace("G0 X1.\n\n(note)\nG0 X2.\n", 2, 3)), vec![3]);
}

#[test]
fn an_unknown_position_is_marked() {
    // G30's second reference position is not in the machine file.
    let t = trace(
        "G21 G90 G54 G0 X0. Y0. Z5.\nG91 G30 Z0.\nG90 G0 X1.\nG0 X1. Y1. Z1.\n",
        2,
        4,
    );
    assert!(t[0].known_before && !t[0].known_after);
    assert!(!t[1].known_before && !t[1].known_after);
    assert!(!t[2].known_before && t[2].known_after);
}

#[test]
fn a_block_with_an_error_says_so() {
    let t = trace(
        "G21 G90 G17 G54 G0 X0. Y0. Z5.\nG1 X#100 F100.\nG1 X1. F100.\n",
        2,
        3,
    );
    assert!(t[0].errors);
    assert!(!t[1].errors);
}

#[test]
fn tracing_does_not_change_the_result() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/golden/basic_mill.nc"
    ))
    .expect("golden program");
    let fanuc = Fanuc::new().expect("dialect");
    let mill = Mill::generic().expect("machine");
    let (traced, t) = interpret_traced(
        src.as_bytes(),
        "test.nc",
        &fanuc,
        &mill,
        Options::default(),
        1..=u32::MAX,
    );
    let plain = common::run_unfiltered(&src, Options::default());
    assert_eq!(traced, plain);
    assert!(!t.is_empty());
    // The trace follows the moves: each block ends where its last move ends.
    let segments = &plain.channels[0].segments;
    for b in t.iter().filter(|b| b.state.work_offset == WorkOffsetId(1)) {
        if let Some(last) = segments.iter().rfind(|s| s.src.block == b.block) {
            let origin = [-380.0, -205.0, -400.0];
            let work: Vec<f64> = (0..3).map(|i| last.end[i] - origin[i]).collect();
            close(xyz(&b.after), [work[0], work[1], work[2]]);
        }
    }
}

mod chained {
    use super::*;
    use proptest::prelude::*;

    /// One move: G90 or G91, an axis and a value.
    fn moves() -> impl Strategy<Value = Vec<(bool, usize, i32)>> {
        proptest::collection::vec((any::<bool>(), 0usize..3, -500i32..500), 1..20)
    }

    proptest! {
        /// Each traced block starts where the one before it ended, and an
        /// incremental move's trace differs from its start by its value.
        #[test]
        fn each_block_starts_where_the_last_one_ended(steps in moves()) {
            let mut src = String::from("G21 G90 G17 G54 G0 X0. Y0. Z0.\n");
            for (incremental, axis, value) in &steps {
                let mode = if *incremental { "G91" } else { "G90" };
                let letter = ['X', 'Y', 'Z'][*axis];
                src.push_str(&format!("{mode} G0 {letter}{:.1}\n", f64::from(*value) / 10.0));
            }
            let n = u32::try_from(steps.len()).unwrap_or(0) + 1;
            let t = trace(&src, 2, n);
            prop_assert_eq!(t.len(), steps.len());
            let mut at = [0.0; 3];
            for (b, (incremental, axis, value)) in t.iter().zip(&steps) {
                prop_assert_eq!(xyz(&b.before), at);
                let v = f64::from(*value) / 10.0;
                at[*axis] = if *incremental { at[*axis] + v } else { v };
                for (x, y) in xyz(&b.after).iter().zip(at) {
                    prop_assert!((x - y).abs() < 1e-9);
                }
                at = xyz(&b.after);
            }
        }
    }
}
