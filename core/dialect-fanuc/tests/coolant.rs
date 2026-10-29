//! M8 turns coolant on before motion; M9 turns it off after.

mod common;

use common::{run, w};
use dialect_api::{Command, CoolantState};

#[test]
fn coolant_on_and_off_around_motion() {
    let r = run("G0 X1. M8\nG0 X2. M9\n");
    assert!(r.diags.is_empty());
    assert_eq!(
        r.commands,
        vec![
            Command::Coolant(CoolantState::Flood),
            Command::Rapid {
                target: vec![w(0, 1.0)]
            },
            Command::Rapid {
                target: vec![w(0, 2.0)]
            },
            Command::Coolant(CoolantState::Off),
        ]
    );
}
