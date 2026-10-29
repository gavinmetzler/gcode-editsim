//! M0/M1 stop and M2/M30 end the program, after the block's motion.

mod common;

use common::{run, w};
use dialect_api::Command;

#[test]
fn stop_codes() {
    let r = run("M0\nM1\nM2\nM30\n");
    assert!(r.diags.is_empty());
    assert_eq!(
        r.commands,
        vec![
            Command::Stop { optional: false },
            Command::Stop { optional: true },
            Command::ProgramEnd { rewind: false },
            Command::ProgramEnd { rewind: true },
        ]
    );
}

#[test]
fn stop_comes_after_motion() {
    let r = run("G0 X1. M0\n");
    assert_eq!(
        r.commands,
        vec![
            Command::Rapid {
                target: vec![w(0, 1.0)]
            },
            Command::Stop { optional: false }
        ]
    );
}

#[test]
fn leading_zeros_do_not_matter() {
    let r = run("M00\nM030\n");
    assert_eq!(
        r.commands,
        vec![
            Command::Stop { optional: false },
            Command::ProgramEnd { rewind: true }
        ]
    );
}
