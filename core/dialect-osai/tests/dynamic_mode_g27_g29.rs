//! G27, G28 and G29 choose continuous or point-to-point movement (manual
//! p. 2-49). G28 is not a reference return, as it is on Fanuc.

mod common;

use common::{abs, run};
use dialect_api::Command;

#[test]
fn g28_moves_like_any_block() {
    let r = run("G71 G28 X10 Y20\nG29 X5\nG27 X1\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    assert_eq!(
        r.moves(),
        vec![
            &Command::Rapid {
                target: vec![abs(0, 10.0), abs(1, 20.0)]
            },
            &Command::Rapid {
                target: vec![abs(0, 5.0)]
            },
            &Command::Rapid {
                target: vec![abs(0, 1.0)]
            },
        ]
    );
    assert!(!r
        .commands
        .iter()
        .any(|c| matches!(c, Command::ReturnToReference { .. })));
}
