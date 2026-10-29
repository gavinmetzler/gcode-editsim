//! Tests for `segments_for_line`.

mod common;

use common::segs_on_lines;
use ir::{segments_for_line, ChannelStream};
use proptest::prelude::*;

#[test]
fn empty_segment_list() {
    assert_eq!(segments_for_line(&[], 1), 0..0);
}

#[test]
fn lines_with_one_or_several_segments() {
    let segs = segs_on_lines(&[1, 1, 1, 3, 3, 7]);
    assert_eq!(segments_for_line(&segs, 1), 0..3);
    assert_eq!(segments_for_line(&segs, 3), 3..5);
    assert_eq!(segments_for_line(&segs, 7), 5..6);
}

#[test]
fn line_with_no_segments_gives_empty_range_at_insertion_point() {
    let segs = segs_on_lines(&[1, 1, 1, 3, 3, 7]);
    // Between existing lines: empty, positioned at the next line's segments.
    assert_eq!(segments_for_line(&segs, 2), 3..3);
    assert_eq!(segments_for_line(&segs, 5), 5..5);
    // Before the first and after the last line.
    assert_eq!(segments_for_line(&segs, 0), 0..0);
    assert_eq!(segments_for_line(&segs, 100), 6..6);
}

#[test]
fn line_with_many_segments() {
    // e.g. an expanded canned cycle: one source line, thousands of segments.
    let mut lines = vec![1, 2];
    lines.extend(std::iter::repeat_n(3, 10_000));
    lines.extend([4, 5]);
    let segs = segs_on_lines(&lines);
    assert_eq!(segments_for_line(&segs, 3), 2..10_002);
    assert_eq!(segments_for_line(&segs, 4), 10_002..10_003);
}

#[test]
fn every_segment_on_one_line() {
    let segs = segs_on_lines(&[9; 50]);
    assert_eq!(segments_for_line(&segs, 9), 0..50);
    assert_eq!(segments_for_line(&segs, 8), 0..0);
    assert_eq!(segments_for_line(&segs, 10), 50..50);
}

#[test]
fn extreme_line_numbers() {
    let segs = segs_on_lines(&[0, u32::MAX]);
    assert_eq!(segments_for_line(&segs, 0), 0..1);
    assert_eq!(segments_for_line(&segs, u32::MAX), 1..2);
}

#[test]
fn channel_stream_method_matches_free_function() {
    let stream = ChannelStream {
        channel: 0,
        segments: segs_on_lines(&[2, 4, 4, 6]),
    };
    assert_eq!(stream.segments_for_line(4), 1..3);
    assert_eq!(stream.segments_for_line(5), 3..3);
}

#[test]
fn unsorted_input_does_not_panic() {
    // Breaks the documented precondition; the answer is unspecified but the
    // range must still be usable for slicing.
    let segs = segs_on_lines(&[5, 1, 9, 1, 3]);
    for line in 0..12 {
        let r = segments_for_line(&segs, line);
        assert!(r.start <= r.end && r.end <= segs.len());
        let _ = &segs[r];
    }
}

proptest! {
    /// Binary search agrees with a simple linear scan on sorted input.
    #[test]
    fn matches_linear_scan(
        mut lines in prop::collection::vec(0u32..50, 0..200),
        query in 0u32..55,
    ) {
        lines.sort_unstable();
        let segs = segs_on_lines(&lines);
        let r = segments_for_line(&segs, query);
        let expected: Vec<usize> = (0..lines.len()).filter(|&i| lines[i] == query).collect();
        prop_assert_eq!(r.clone().collect::<Vec<_>>(), expected);
        // Empty ranges sit at the insertion point.
        prop_assert_eq!(r.start, lines.partition_point(|&l| l < query));
    }
}
