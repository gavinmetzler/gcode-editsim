//! Feed, spindle, tool and work offset recorded on each segment.

mod common;

use common::{kinds, moves, run, segments};
use ir::{LengthUnit, SegKind};

#[test]
fn feed_only_on_feed_moves() {
    let p = run("G1 X1. F250.\nG0 X2.\nG1 X3.\n");
    let feeds: Vec<f64> = moves(&p).iter().map(|s| s.feed).collect();
    assert_eq!(feeds, vec![250.0, 0.0, 250.0]);
}

#[test]
fn spindle_speed_shown_only_while_running() {
    let p = run("S1000\nG0 X1.\nM3\nG0 X2.\nM5\nG0 X3.\n");
    let rpm: Vec<f64> = moves(&p).iter().map(|s| s.spindle_rpm).collect();
    assert_eq!(rpm, vec![0.0, 1000.0, 0.0]);
}

#[test]
fn tool_change_segment_and_tools_used() {
    let p = run("T2 M6\nG0 X1.\nT5 M6\nG0 X2.\nT2 M6\n");
    assert_eq!(
        kinds(&p),
        vec![
            SegKind::ToolChange,
            SegKind::Rapid,
            SegKind::ToolChange,
            SegKind::Rapid,
            SegKind::ToolChange
        ]
    );
    let tools: Vec<u32> = segments(&p).iter().map(|s| s.tool).collect();
    assert_eq!(tools, vec![2, 2, 5, 5, 2]);
    assert_eq!(p.meta.tools_used, vec![2, 5]);
    // A tool change does not move.
    let tc = &segments(&p)[2];
    assert_eq!(tc.start, tc.end);
}

#[test]
fn units_used_in_order() {
    let p = run("G20\nG21\nG20\n");
    assert_eq!(
        p.meta.units_used,
        vec![LengthUnit::Metric, LengthUnit::Inch]
    );
}

#[test]
fn source_references_follow_lines_and_blocks() {
    let p = run("(header)\n\nG0 X1.\nG0 X2.\n");
    let srcs: Vec<(u32, u32)> = moves(&p)
        .iter()
        .map(|s| (s.src.line, s.src.block))
        .collect();
    // Blank lines produce no block; comment lines do.
    assert_eq!(srcs, vec![(3, 1), (4, 2)]);
    assert_eq!(p.files, vec!["test.nc".to_owned()]);
    assert_eq!(ir::segments_for_line(segments(&p), 4), 1..2);
}

#[test]
fn bounding_box_in_meta() {
    let p = run("G0 X0 Y0 Z0\nG1 X10. F100.\n");
    let b = p.meta.bbox.expect("motion");
    // The first move starts at machine zero.
    assert!((b.min[0] - -380.0).abs() < 1e-9);
    assert!((b.max[0] - 0.0).abs() < 1e-9);
    assert!((b.min[2] - -400.0).abs() < 1e-9);
}
