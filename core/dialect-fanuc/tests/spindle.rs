//! S sets the speed; M3/M4 start the spindle before motion, M5 stops it
//! after.

mod common;

use common::{run, w};
use dialect_api::{Command, SpindleState};

fn state(s: SpindleState) -> Command {
    Command::SetSpindleState {
        spindle: 0,
        state: s,
    }
}

#[test]
fn speed_and_direction() {
    let r = run("S1500 M3\nM4\nM5\n");
    assert!(r.diags.is_empty());
    assert_eq!(
        r.commands,
        vec![
            Command::SetSpindleSpeed {
                spindle: 0,
                rpm: 1500.0
            },
            state(SpindleState::Cw),
            state(SpindleState::Ccw),
            state(SpindleState::Off),
        ]
    );
}

#[test]
fn start_before_motion_stop_after() {
    let r = run("G0 X1. M3\nG0 X2. M5\n");
    assert_eq!(
        r.commands,
        vec![
            state(SpindleState::Cw),
            Command::Rapid {
                target: vec![w(0, 1.0)]
            },
            Command::Rapid {
                target: vec![w(0, 2.0)]
            },
            state(SpindleState::Off),
        ]
    );
}

#[test]
fn speed_without_start_does_not_start() {
    let r = run("S800\n");
    assert_eq!(
        r.commands,
        vec![Command::SetSpindleSpeed {
            spindle: 0,
            rpm: 800.0
        }]
    );
}

#[test]
fn negative_speed_is_rejected() {
    let r = run("S-5 M3\n");
    assert!(r.commands.is_empty());
    assert_eq!(r.errors().len(), 1);
}
