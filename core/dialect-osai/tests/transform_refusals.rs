//! Which blocks a code transformation must refuse on the 10 Series
//! (M4.9a): codes whose effect a rewrite of the axis words cannot carry.

mod common;

use common::xyz;
use dialect_api::{BlockContext, Dialect};
use dialect_osai::Osai;
use ir::{Diagnostics, SourceRef};

fn refusals(src: &str) -> Vec<Option<String>> {
    let osai = Osai::new().expect("dialect");
    let axes = xyz();
    let mut state = osai.initial_state(&axes);
    let program = gcode_syntax::parse_str(src);
    let mut out = Vec::new();
    program
        .blocks
        .iter()
        .map(|block| {
            let ctx = BlockContext {
                axes: &axes,
                src: SourceRef::default(),
            };
            out.clear();
            osai.interpret_block(block, &ctx, &mut state, &mut out, &mut Diagnostics::new());
            osai.transform_refusal(block, &state)
        })
        .collect()
}

#[test]
fn ordinary_moves_and_whole_origins_can_be_transformed() {
    let r = refusals("G71 G90\nG17\n(UAO,1)\nG0 X0 Y0 Z5\nG1 Z-1 F100\nG2 X10 Y0 I5 J0\nG3 X20 Y0 R5\n(DIS,\"cut\")\nM5\n");
    assert!(r.iter().all(Option::is_none), "{r:?}");
}

#[test]
fn presetting_cycles_and_home_coordinates_are_refused() {
    for (code, word) in [
        ("G92 X0 Y0", "G92"),
        ("G98", "G98"),
        ("G99", "G99"),
        ("G79 G0 Z0", "G79"),
    ] {
        let r = refusals(&format!("G71 G90\n{code}\n"));
        let why = r[1].clone().unwrap_or_else(|| panic!("{code} is refused"));
        assert!(why.contains(word), "{code}: {why}");
    }
    let r = refusals("G71 G90 G17\nG81 X10 Y10 Z-5 R2 F100\nX20\nG80\nG0 X0\n");
    assert!(
        r[1].as_deref().is_some_and(|w| w.contains("canned cycle")),
        "{r:?}"
    );
    assert!(
        r[2].as_deref().is_some_and(|w| w.contains("canned cycle")),
        "{r:?}"
    );
    assert_eq!(r[3], None);
}

#[test]
fn commands_that_shift_or_turn_coordinates_are_refused() {
    for cmd in [
        "(UAO,1,Z)",
        "(ROT,45)",
        "(SCF,2)",
        "(MIR,X)",
        "(UTO,1)",
        "(UIO,X10)",
    ] {
        let r = refusals(&format!("G71 G90\n{cmd}\n"));
        let why = r[1].clone().unwrap_or_else(|| panic!("{cmd} is refused"));
        assert!(why.contains(&cmd[1..4]), "{cmd}: {why}");
    }
}
