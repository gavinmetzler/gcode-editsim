//! Which blocks a code transformation must refuse (M4.6): codes whose
//! effect a rewrite of the axis words cannot carry over.

mod common;

use common::xyz;
use dialect_api::{BlockContext, Dialect};
use dialect_fanuc::Fanuc;
use ir::{Diagnostics, SourceRef};

/// The refusal for each line of `src`, run in order (each line's state is
/// the state after it).
fn refusals(src: &str) -> Vec<Option<String>> {
    let fanuc = Fanuc::new().expect("dialect");
    let axes = xyz();
    let mut state = fanuc.initial_state(&axes);
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
            fanuc.interpret_block(block, &ctx, &mut state, &mut out, &mut Diagnostics::new());
            fanuc.transform_refusal(block, &state)
        })
        .collect()
}

#[test]
fn ordinary_moves_can_be_transformed() {
    let r = refusals(
        "G21 G90 G17 G54 G0 X0. Y0. Z5.\nG1 Z-1. F100.\nG2 X10. Y0. I5. J0.\nG91 G1 X5.\nM5\n",
    );
    assert!(r.iter().all(Option::is_none), "{r:?}");
}

#[test]
fn coordinate_shifts_are_refused() {
    for code in [
        "G92 X0. Y0.",
        "G52 X10.",
        "G10 L2 P1 X0.",
        "G68 X0. Y0. R45.",
        "G51 X0. Y0. P2000",
        "G16",
        "G65 P9010",
    ] {
        let r = refusals(&format!("G21 G90 G0 X1.\n{code}\n"));
        let why = r[1].clone().unwrap_or_else(|| panic!("{code} is refused"));
        assert!(
            why.contains(code.split(' ').next().unwrap_or_default()),
            "{code}: {why}"
        );
    }
}

#[test]
fn canned_cycles_are_refused_while_active() {
    let r =
        refusals("G21 G90 G17 G0 X0. Y0. Z5.\nG81 X10. Y10. Z-5. R2. F100.\nX20.\nG80\nG0 X0.\n");
    assert!(
        r[1].as_deref().is_some_and(|w| w.contains("canned cycle")),
        "{r:?}"
    );
    assert!(
        r[2].as_deref().is_some_and(|w| w.contains("canned cycle")),
        "the next hole too"
    );
    assert_eq!(r[3], None, "G80 cancels it");
    assert_eq!(r[4], None);
}

#[test]
fn machine_coordinates_and_reference_returns_are_refused() {
    let r = refusals("G21 G90 G0 X1.\nG53 G0 Z0.\nG91 G28 Z0.\nG30 Z0.\n");
    assert!(r[1].as_deref().is_some_and(|w| w.contains("G53")), "{r:?}");
    assert!(r[2].as_deref().is_some_and(|w| w.contains("G28")), "{r:?}");
    assert!(r[3].as_deref().is_some_and(|w| w.contains("G30")), "{r:?}");
}

#[test]
fn a_subprogram_call_is_not_refused() {
    // It gets a warning from the transformation instead (M4_PLAN §3).
    let r = refusals("G21 G90 G0 X1.\nM98 P10\n");
    assert_eq!(r[1], None);
}
