//! Full circles must survive floating-point drift in the start position.
//!
//! Regression test for review finding C3: after many incremental moves the
//! start point differs from the absolute end point by ~1e-11 mm, and a G2
//! full circle collapsed to a near-zero arc.

mod common;

use common::{moves, run};
use ir::bounding_box;

fn program(dir: &str, steps: usize, radius: f64) -> String {
    let mut s = String::from("G21 G90 G54 G17 G0 X0. Y0. Z0.\nG91 G1 X0.1 F100.\n");
    for _ in 1..steps {
        s.push_str("X0.1\n");
    }
    let end = 0.1 * steps as f64;
    s.push_str(&format!("G90 {dir} X{end:.1} Y0. J{radius:.3}\n"));
    s
}

#[test]
fn full_circle_after_incremental_drift_is_a_full_circle() {
    for dir in ["G2", "G3"] {
        for (steps, radius) in [(1000, 5.0), (3, 0.004), (500, 0.5), (2000, 25.0)] {
            let p = run(&program(dir, steps, radius));
            let arc = *moves(&p).last().unwrap();
            assert!(arc.arc.is_some(), "{dir} {steps}: not an arc");
            let b = bounding_box(std::iter::once(arc)).unwrap();
            let height = b.max[1] - b.min[1];
            assert!(
                (height - 2.0 * radius).abs() < 1e-6,
                "{dir} after {steps} steps, r={radius}: box height {height}, expected {}",
                2.0 * radius
            );
            // The circle ends exactly where it started.
            assert_eq!(arc.start[0], arc.end[0], "{dir} {steps}");
        }
    }
}
