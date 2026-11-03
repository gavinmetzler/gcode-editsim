//! G92 (workpiece coordinate system setting) is supported (M5.6e, owner,
//! 2026-11-03): the tool's current position becomes the programmed values,
//! by a shift of every work coordinate system; nothing moves. G92.1 cancels
//! the shift. Real router programs start with it
//! (`G54 G92 X1000. Y1250. Z0`).

mod common;

use common::{run, w};
use dialect_api::Command;
use ir::DiagCode;

fn presets(r: &common::Run) -> Vec<&Command> {
    r.commands
        .iter()
        .filter(|c| matches!(c, Command::Preset { .. } | Command::CancelPreset))
        .collect()
}

#[test]
fn g92_presets_the_named_axes_and_is_not_a_move() {
    let r = run("G21 G90 G17\nG0 X1.\nG54 G92 X1000. Y1250. Z0\nG0 X2.\n");
    assert_eq!(
        presets(&r),
        vec![&Command::Preset {
            target: vec![w(0, 1000.0), w(1, 1250.0), w(2, 0.0)]
        }]
    );
    assert_eq!(r.moves().len(), 2, "{:?}", r.commands);
}

#[test]
fn g92_gives_no_message() {
    let r = run("G21 G90 G17\nG92 X0 Y0\n");
    assert!(
        r.diags.iter().all(|d| d.code != DiagCode::UnsupportedGCode),
        "{:?}",
        r.diags
    );
}

#[test]
fn its_values_are_positions_even_in_incremental_mode() {
    let r = run("G21 G91 G17\nG92 X10.\n");
    assert_eq!(
        presets(&r),
        vec![&Command::Preset {
            target: vec![w(0, 10.0)]
        }]
    );
}

#[test]
fn its_values_are_in_the_program_units() {
    let r = run("G20 G90 G17\nG92 X1.\n");
    let Command::Preset { target } = presets(&r)[0] else {
        panic!("a preset");
    };
    assert!((target[0].value - 25.4).abs() < 1e-9, "{target:?}");
}

#[test]
fn g92_1_cancels_the_preset() {
    let r = run("G21 G90 G17\nG92 X0\nG92.1\n");
    assert_eq!(presets(&r).last(), Some(&&Command::CancelPreset));
}
