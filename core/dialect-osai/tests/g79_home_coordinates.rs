//! G79: coordinates in this block refer to the home position (manual
//! p. 2-80), for one block only.

mod common;

use common::{abs, run};
use dialect_api::Command;
use ir::DiagCode;

#[test]
fn g79_rapid_is_in_machine_coordinates_for_one_block() {
    let r = run("G71 (UAO,1)\nG79 X70 Y55\nX30\n");
    assert_eq!(
        r.moves(),
        vec![
            &Command::MachineRapid {
                target: vec![abs(0, 70.0), abs(1, 55.0)]
            },
            &Command::Rapid {
                target: vec![abs(0, 30.0)]
            },
        ]
    );
}

#[test]
fn g79_is_absolute_even_in_g91() {
    let r = run("G71 G91\nG79 X70\n");
    assert_eq!(
        r.moves(),
        vec![&Command::MachineRapid {
            target: vec![abs(0, 70.0)]
        }]
    );
}

#[test]
fn g79_with_a_feed_move_is_not_simulated() {
    let r = run("G71 F100\nG79 G1 X70\n");
    assert!(r.moves().is_empty());
    assert!(r.has(DiagCode::NotYetSimulated, "G79 with G1"));
    assert!(r.position_lost());
}
