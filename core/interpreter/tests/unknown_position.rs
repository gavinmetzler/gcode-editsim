//! After the machine moves somewhere that is not simulated, later moves are
//! not drawn from a stale position.

mod common;

use common::{at, moves, run};
use ir::DiagCode;

#[test]
fn g30_leaves_the_axis_unknown_until_an_absolute_move() {
    let p = run("G21 G90 G54 G0 X10. Y10. Z5.\nG91 G30 P2 Z0.\nG90 X20.\nZ10.\nX30.\n");
    let m = moves(&p);
    // X20. is not drawn (Z unknown); Z10. makes Z known but starts from an
    // unknown Z, so it is not drawn either; X30. is drawn from (X20, Z10).
    let last = m.last().unwrap();
    at(&last.start, [-360.0, -195.0, -390.0]);
    at(&last.end, [-350.0, -195.0, -390.0]);
    assert_eq!(m.len(), 2, "{m:#?}");
    let warnings: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagCode::NotYetSimulated)
        .collect();
    assert_eq!(warnings.len(), 1, "{:?}", p.diagnostics);
    assert!(warnings[0].message.contains("position of Z is unknown"));
    assert!(
        warnings[0]
            .message
            .contains("reference position 2 on line 2"),
        "{}",
        warnings[0].message
    );
}

#[test]
fn incremental_moves_do_not_make_an_unknown_axis_known() {
    let p = run("G21 G90 G54 G0 X0. Y0. Z5.\nG91 G30 P2 Z0.\nG91 Z-5.\nX1.\n");
    // Z is still unknown, so nothing after G30 is drawn.
    assert_eq!(moves(&p).len(), 1);
}

#[test]
fn moves_after_an_unknown_position_wait_for_a_known_one() {
    // G30 goes to a reference position the machine file does not give.
    let src =
        "G21 G90 G54 G0 X0. Y0. Z5.\nG91 G30 X0. Y0. Z0.\nG90 G0 X5.\nG0 X1. Y1. Z5.\nG0 X2.\n";
    let p = run(src);
    let lines: Vec<u32> = moves(&p).iter().map(|m| m.src.line).collect();
    // Line 3 sets only X, so Y and Z are still unknown: it is not drawn.
    // Line 4 sets every axis, and from there everything is drawn.
    assert!(!lines.contains(&3), "{lines:?}");
    assert_eq!(lines.last(), Some(&5));
    let m = moves(&p);
    at(&m[m.len() - 1].start, [-379.0, -204.0, -395.0]);
}

#[test]
fn a_call_to_a_program_not_in_the_file_does_not_hide_later_moves() {
    // Moves are never hidden (owner decision, 2026-10-31): the call is
    // reported and skipped, and later moves are drawn.
    let p = run("G21 G90 G54 G0 X0. Y0. Z5.\nM98 P1000\nG0 X0. Y0. Z5.\nX10.\n");
    assert_eq!(moves(&p).len(), 3);
    assert!(p
        .diagnostics
        .iter()
        .any(|d| d.code == DiagCode::NotYetSimulated && d.message.contains("O1000")));
}

#[test]
fn machine_without_reference_positions_makes_g28_unknown() {
    use dialect_fanuc::Fanuc;
    use interpreter::{interpret, Options};
    use machine_mill::Mill;
    let mill = Mill::from_toml_str(
        "[machine]\nname = \"m\"\nfamily = \"mill\"\n\
         [[axis]]\nletter = \"X\"\nkind = \"linear\"\narc_offset = \"I\"\n\
         [[axis]]\nletter = \"Y\"\nkind = \"linear\"\narc_offset = \"J\"\n\
         [[axis]]\nletter = \"Z\"\nkind = \"linear\"\narc_offset = \"K\"\n",
    )
    .unwrap();
    let src = b"G21 G90 G0 X0. Y0. Z5.\nG91 G28 Z0.\nG90 X10.\n";
    let p = interpret(
        src,
        "t.nc",
        &Fanuc::new().unwrap(),
        &mill,
        Options::default(),
    );
    let errors: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagCode::InvalidValue)
        .collect();
    assert!(errors
        .iter()
        .any(|d| d.message.contains("no reference position for Z")));
    // The move after G28 is not drawn.
    assert_eq!(p.channels[0].segments.len(), 1);
}

#[test]
fn simulation_starts_at_the_reference_position() {
    let p = run("G21 G90 G54 G0 X0.\n");
    at(&moves(&p)[0].start, [0.0, 0.0, 0.0]);
}
