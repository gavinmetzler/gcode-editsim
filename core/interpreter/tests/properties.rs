//! Whatever the input, the interpreter does not panic and its output keeps
//! the IR's guarantees.

mod common;

use common::{run, segments};
use proptest::prelude::*;

fn gcode_like() -> impl Strategy<Value = String> {
    proptest::string::string_regex(
        r"(G(0|1|2|3|17|18|19|20|21|54|55|56|90|91)|M(3|5|6|30)|[XYZIJKR]-?[0-9]{1,4}\.[0-9]{0,3}|[FST][0-9]{1,4}|/| |\n)*",
    )
    .expect("valid regex")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn never_panics_and_keeps_program_order(src in gcode_like()) {
        let p = run(&src);
        let segs = segments(&p);
        // Segments are in program order (non-decreasing line numbers), so
        // line lookup by binary search is valid.
        prop_assert!(segs.windows(2).all(|w| w[0].src.line <= w[1].src.line));
        // Each segment starts where the previous one ended.
        prop_assert!(segs.windows(2).all(|w| w[0].end == w[1].start));
        for s in segs {
            prop_assert!(s.start.iter().chain(&s.end).all(|v| v.is_finite()));
        }
    }

    #[test]
    fn never_panics_on_arbitrary_text(src in any::<String>()) {
        let _ = run(&src);
    }
}
