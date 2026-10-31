//! G70 is inches and G71 millimetres (manual p. 2-79). G20 and G21 are GTL
//! profile codes on the 10 Series, not units.

mod common;

use common::{abs, run};
use dialect_api::Command;
use ir::{DiagCode, LengthUnit};

#[test]
fn g70_converts_lengths_and_feed() {
    let r = run("G70 G1 X1 F10\n");
    assert!(r.commands.contains(&Command::SetUnits(LengthUnit::Inch)));
    assert!(r.commands.contains(&Command::SetFeedRate(254.0)));
    assert_eq!(
        r.moves(),
        vec![&Command::Linear {
            target: vec![abs(0, 25.4)]
        }]
    );
}

#[test]
fn g20_and_g21_are_not_units() {
    // Unknown here: reported and ignored; later moves are still drawn
    // (moves are never hidden, owner decision 2026-10-31).
    let r = run("G21\nG0 X1 Y1 Z1\n");
    assert!(r.has(DiagCode::UnsupportedGCode, "G21 is not supported"));
    assert_eq!(r.moves().len(), 1);
}

#[test]
fn rotary_axes_are_never_converted() {
    let ax = common::axes(&[('X', false), ('Y', false), ('Z', false), ('B', true)]);
    let r = common::run_on("G70 B90\n", &ax);
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![abs(3, 90.0)]
        }]
    );
}
