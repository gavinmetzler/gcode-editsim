//! G92/G98 preset the axes until G99 cancels them (manual p. 2-82: "When
//! the axis value is entered in a G92 or G98 block it becomes the current
//! axis position"). Supported since M5.6e (owner, 2026-11-03). G98 also
//! takes mirroring into account, which is not simulated, so it acts like
//! G92. G99 must be alone in its block (compatibility table, p. 1-10).

mod common;

use common::{abs, run};
use dialect_api::Command;
use ir::DiagCode;

fn presets(r: &common::Run) -> Vec<&Command> {
    r.commands
        .iter()
        .filter(|c| matches!(c, Command::Preset { .. } | Command::CancelPreset))
        .collect()
}

#[test]
fn g92_presets_the_named_axes_without_a_message() {
    // G17 must be alone in its block (p. 2-47).
    let r = run("G17\nG71 G0 X100 Y80\nG92 X0 Y0\nG0 X10\nG99\nG0 X1 Y1 Z1\n");
    assert_eq!(
        presets(&r),
        vec![
            &Command::Preset {
                target: vec![abs(0, 0.0), abs(1, 0.0)]
            },
            &Command::CancelPreset
        ]
    );
    assert!(
        r.diags.iter().all(|d| d.code != DiagCode::UnsupportedGCode),
        "{:?}",
        r.diags
    );
    // The preset is not a move.
    assert_eq!(r.moves().len(), 3);
}

#[test]
fn g98_presets_like_g92() {
    let r = run("G17\nG71\nG98 X5\n");
    assert_eq!(
        presets(&r),
        vec![&Command::Preset {
            target: vec![abs(0, 5.0)]
        }]
    );
}

#[test]
fn g99_must_be_alone() {
    let r = run("G99 X0\n");
    assert!(r.has(DiagCode::ModalGroupConflict, "G99 must be alone"));
}
