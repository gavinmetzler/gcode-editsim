//! A lower-case r rounds the corner to the next move (manual, U-axis
//! example p. 2-28). The parser reads it as R; outside an arc it is
//! reported and the position treated as unknown, not ignored.

mod common;

use common::run;
use ir::DiagCode;

#[test]
fn r_outside_an_arc_hides_the_move() {
    for src in ["G71 G1 X10 r5 F100\n", "N130 r5\n"] {
        let r = run(src);
        assert!(
            r.has(DiagCode::UnsupportedWord, "R outside an arc"),
            "{src}"
        );
        assert!(r.moves().is_empty(), "{src}");
        assert!(r.position_lost(), "{src}");
    }
}
