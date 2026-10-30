//! Lines in 10 Series syntax the shared parser cannot read (labels, axis
//! names without a value, E-parameter assignments, ">>") are reported, and
//! the position is treated as unknown because the control may have moved.

mod common;

use common::run;
use ir::DiagCode;

#[test]
fn each_unreadable_line_is_reported_and_the_position_is_lost() {
    for line in [
        "\"START\" N125 X0\n",
        "N1 G X Y\n",
        "E10=123.5\n",
        "G90 G1 X80 Y>>35 F100\n",
    ] {
        let r = run(line);
        assert!(r.codes().contains(&DiagCode::SyntaxError), "{line}");
        assert!(r.position_lost(), "{line}");
        assert!(r.moves().is_empty(), "{line}");
    }
}
