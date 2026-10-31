//! Unsupported codes that change how *later* blocks are interpreted must
//! stop later motion from being shown, not just their own block.
//!
//! Regression test for review finding C1.

mod common;

use common::{run, w};
use dialect_api::Command;
use ir::DiagCode;

#[test]
fn coordinate_changing_codes_hide_later_moves() {
    for (code, cancel) in [
        ("G52 X10. Y10.", Some("G52 X0 Y0")),
        ("G68 X0 Y0 R90.", Some("G69")),
        ("G51 X0 Y0 P2", Some("G50")),
        ("G51.1 X0.", Some("G50.1")),
        ("G16", Some("G15")),
        ("G10 L2 P1 X-100.", None),
    ] {
        let r = run(&format!("G0 X1.\n{code}\nG0 X2.\nG1 X3. F100.\n"));
        assert_eq!(
            r.moves(),
            vec![&Command::Rapid {
                target: vec![w(0, 1.0)]
            }],
            "after {code}"
        );
        assert!(r.codes().contains(&DiagCode::UnsupportedGCode), "{code}");
        // Explained once, at the code itself, not on every later block.
        assert_eq!(r.errors().len(), 1, "{code}: {:?}", r.diags);
        assert_eq!(r.errors()[0].src.line, 2, "{code}");

        if let Some(cancel) = cancel {
            let r = run(&format!("{code}\nG0 X2.\n{cancel}\nG0 X3.\n"));
            assert_eq!(
                r.moves(),
                vec![&Command::Rapid {
                    target: vec![w(0, 3.0)]
                }],
                "{code} cancelled by {cancel}"
            );
        }
    }
}

#[test]
fn unknown_g_code_hides_later_moves() {
    // G33 (threading) is not in the table; what it does to later blocks is
    // unknown, so nothing after it is shown.
    let r = run("G0 X1.\nG33 Z-10. F1.5\nZ-20.\nG0 X5.\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 1.0)]
        }]
    );
    assert_eq!(r.errors().len(), 1, "{:?}", r.diags);
}

#[test]
fn subprogram_call_hides_later_moves() {
    let r = run("G0 X1.\nM98 P1000\nG0 X2.\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 1.0)]
        }]
    );
    assert_eq!(r.codes(), vec![DiagCode::UnsupportedMCode]);
}

#[test]
fn non_motion_commands_still_apply_while_moves_are_hidden() {
    let r = run("G10 L2 P1 X0\nM5\nM30\n");
    assert!(r.commands.contains(&Command::ProgramEnd { rewind: true }));
}

#[test]
fn harmless_path_modes_do_not_hide_moves() {
    // Exact-stop / cutting-mode and look-ahead codes do not change the path.
    let r = run("G64\nG0 X1.\nG61 G0 X2.\nG05.1 Q1\nG0 X3.\nG09 G0 X4.\n");
    assert_eq!(r.moves().len(), 4, "{:?}", r.diags);
    assert!(r.errors().is_empty(), "{:?}", r.diags);
}
