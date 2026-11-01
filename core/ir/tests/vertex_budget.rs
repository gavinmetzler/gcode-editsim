//! Keeping the drawing within a vertex budget: a crafted program of huge
//! arcs must not make the render buffer exhaust memory (M3 review).

mod common;

use common::{arc, line_seg, pos, seg};
use ir::render::{fit_tolerance, vertex_count};
use ir::{tessellate, tessellation_len, ChannelStream, Program, SegKind, Segment};

fn program(segments: Vec<Segment>) -> Program {
    Program {
        channels: vec![ChannelStream {
            channel: 0,
            segments,
        }],
        ..Program::default()
    }
}

/// A full circle of radius `r` around the origin, starting at (r, 0).
fn circle(r: f64) -> Segment {
    arc(SegKind::ArcCw, &[r, 0.0], &[r, 0.0], &[0.0, 0.0], r)
}

#[test]
fn the_length_matches_the_points_tessellate_gives() {
    let segs = [
        line_seg(&[0.0, 0.0], &[3.0, 4.0]),
        seg(SegKind::Dwell, pos(&[1.0]), pos(&[1.0]), 1),
        circle(10.0),
        circle(100_000.0),
        arc(
            SegKind::ArcCcw,
            &[10.0, 0.0],
            &[0.0, 10.0],
            &[0.0, 0.0],
            10.0,
        ),
    ];
    for s in &segs {
        for tol in [0.001, 0.01, 1.0, 1e6, 0.0, f64::NAN] {
            assert_eq!(
                tessellation_len(s, tol),
                tessellate(s, tol).count(),
                "{s:?} at {tol}"
            );
        }
    }
}

#[test]
fn a_program_within_the_budget_keeps_its_tolerance() {
    let p = program(vec![circle(10.0), line_seg(&[0.0], &[5.0])]);
    assert_eq!(fit_tolerance(&p, 0.01, 1_000_000), 0.01);
}

#[test]
fn huge_arcs_get_a_coarser_tolerance_that_fits_the_budget() {
    let p = program((0..200).map(|_| circle(100_000.0)).collect());
    let budget = 50_000;
    assert!(vertex_count(&p, 0.01) > budget);
    let tol = fit_tolerance(&p, 0.01, budget);
    assert!(tol > 0.01, "{tol}");
    assert!(vertex_count(&p, tol) <= budget, "{}", vertex_count(&p, tol));
}

#[test]
fn a_budget_that_cannot_be_met_gives_the_coarsest_drawing() {
    // Every circle needs at least 5 points (one every 90 degrees): 200
    // circles cannot fit in 100. The result is as coarse as it gets.
    let p = program((0..200).map(|_| circle(10.0)).collect());
    let tol = fit_tolerance(&p, 0.01, 100);
    assert_eq!(vertex_count(&p, tol), 200 * 5);
}
