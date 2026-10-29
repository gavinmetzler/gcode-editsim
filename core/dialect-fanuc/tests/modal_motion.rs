//! The motion mode (G0/G1/G2/G3) stays in effect until another replaces it.

mod common;

use common::{run, w};
use dialect_api::Command;

#[test]
fn axis_words_alone_use_the_last_motion_mode() {
    let r = run("G0 X1.\nY2.\nG1 F100. X3.\nY4.\nG0 Z5.\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    assert_eq!(
        r.moves(),
        vec![
            &Command::Rapid {
                target: vec![w(0, 1.0)]
            },
            &Command::Rapid {
                target: vec![w(1, 2.0)]
            },
            &Command::Linear {
                target: vec![w(0, 3.0)]
            },
            &Command::Linear {
                target: vec![w(1, 4.0)]
            },
            &Command::Rapid {
                target: vec![w(2, 5.0)]
            },
        ]
    );
}

#[test]
fn power_on_mode_is_rapid() {
    let r = run("X10.\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 10.0)]
        }]
    );
}

#[test]
fn motion_code_without_axes_only_sets_the_mode() {
    let r = run("G1 F200.\nX1.\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Linear {
            target: vec![w(0, 1.0)]
        }]
    );
}

#[test]
fn several_axes_in_written_order() {
    let r = run("G0 Z5. X1. Y2.\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(2, 5.0), w(0, 1.0), w(1, 2.0)]
        }]
    );
}
