//! Rotate (M4.7): the selected lines are turned about a centre in a plane
//! (counter-clockwise looking down the plane's third axis, like the arc
//! planes); arc centre offsets turn with them; R is unchanged.

use dialect_fanuc::Fanuc;
use interpreter::Options;
use ir::{ArcPlane, Pos, MAX_AXES};
use machine_mill::Mill;
use transform::{apply_edits, transform, Note, TransformError, Transformation, Transformed};

const XY: ArcPlane = ArcPlane {
    first: 0,
    second: 1,
};
const ZX: ArcPlane = ArcPlane {
    first: 2,
    second: 0,
};

fn rotate(plane: ArcPlane, angle: f64, centre: [f64; 3]) -> Transformation {
    let mut c: Pos = [0.0; MAX_AXES];
    c[..3].copy_from_slice(&centre);
    Transformation::Rotate {
        plane,
        angle,
        centre: c,
    }
}

fn run(
    src: &str,
    first: u32,
    last: u32,
    t: &Transformation,
) -> Result<Transformed, TransformError> {
    let fanuc = Fanuc::new().expect("dialect");
    let mill = Mill::generic().expect("machine");
    transform(
        src.as_bytes(),
        &fanuc,
        &mill,
        Options::default(),
        first..=last,
        t,
    )
}

fn rotated(src: &str, first: u32, last: u32, t: &Transformation) -> (String, Vec<Note>) {
    let result = run(src, first, last, t).unwrap_or_else(|e| panic!("refused: {e}"));
    let text = apply_edits(src.as_bytes(), &result.edits).expect("edits apply");
    (String::from_utf8(text).expect("utf-8"), result.notes)
}

fn refused(src: &str, first: u32, last: u32, t: &Transformation) -> (u32, String) {
    match run(src, first, last, t) {
        Err(TransformError::Refused { line, reason }) => (line, reason),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

const HEAD: &str = "G21 G90 G17 G54 G0 X0. Y0. Z5.\n";

#[test]
fn absolute_moves_turn_about_the_centre() {
    let src = format!("{HEAD}G1 X10. Y0. F100.\nX10. Y5.\n");
    let (out, _) = rotated(&src, 2, 3, &rotate(XY, 90.0, [0.0, 0.0, 0.0]));
    assert_eq!(out, format!("{HEAD}G1 X0. Y10. F100.\nX-5. Y10.\n"));
    let (out, _) = rotated(&src, 2, 3, &rotate(XY, 180.0, [5.0, 5.0, 0.0]));
    assert_eq!(out, format!("{HEAD}G1 X0. Y10. F100.\nX0. Y5.\n"));
}

#[test]
fn turning_about_z_adds_the_y_word_a_move_now_needs() {
    let src = format!("{HEAD}G1 X10. F100.\n");
    let (out, _) = rotated(&src, 2, 2, &rotate(XY, 90.0, [0.0, 0.0, 0.0]));
    assert_eq!(out, format!("{HEAD}G1 X0. Y10. F100.\n"));
}

#[test]
fn angles_that_are_not_quarter_turns_round_to_the_increment() {
    let src = format!("{HEAD}G1 X10. Y0. F100.\n");
    let (out, _) = rotated(&src, 2, 2, &rotate(XY, 30.0, [0.0, 0.0, 0.0]));
    assert_eq!(out, format!("{HEAD}G1 X8.66 Y5. F100.\n"));
}

#[test]
fn incremental_moves_are_turned() {
    let src = format!("{HEAD}G91 G1 X10. F100.\nX10.\nG90 G0 Z50.\n");
    let (out, _) = rotated(&src, 2, 3, &rotate(XY, 90.0, [0.0, 0.0, 0.0]));
    assert_eq!(
        out,
        format!("{HEAD}G91 G1 X0. Y10. F100.\nX0. Y10.\nG90 G0 Z50.\n")
    );
}

#[test]
fn arc_centre_offsets_turn_and_r_stays() {
    let src = format!("{HEAD}G1 X10. Y0. F100.\nG2 X20. Y0. I5. J0.\nG3 X30. Y0. R5.\n");
    let (out, _) = rotated(&src, 2, 4, &rotate(XY, 90.0, [0.0, 0.0, 0.0]));
    assert_eq!(
        out,
        format!("{HEAD}G1 X0. Y10. F100.\nG2 X0. Y20. I0. J5.\nG3 X0. Y30. R5.\n")
    );
}

#[test]
fn a_centre_offset_that_was_not_written_is_added() {
    let src = format!("{HEAD}G1 X10. Y0. F100.\nG2 X20. Y0. I5.\n");
    let (out, _) = rotated(&src, 2, 3, &rotate(XY, 90.0, [0.0, 0.0, 0.0]));
    assert_eq!(
        out,
        format!("{HEAD}G1 X0. Y10. F100.\nG2 X0. Y20. I0. J5.\n")
    );
}

#[test]
fn helices_keep_their_depth() {
    let src = format!("{HEAD}G1 X10. Y0. Z0. F100.\nG2 X10. Y0. Z-2. I-5. J0.\n");
    let (out, _) = rotated(&src, 2, 3, &rotate(XY, 90.0, [0.0, 0.0, 0.0]));
    assert_eq!(
        out,
        format!("{HEAD}G1 X0. Y10. Z0. F100.\nG2 X0. Y10. Z-2. I0. J-5.\n")
    );
}

#[test]
fn other_planes_work_the_same_way() {
    // G18: Z then X; counter-clockwise looking down Y.
    let src = "G21 G90 G18 G54 G0 X0. Y0. Z0.\nG1 X0. Z10. F100.\nG2 X0. Z20. K5. I0.\n";
    let (out, _) = rotated(src, 2, 3, &rotate(ZX, 90.0, [0.0, 0.0, 0.0]));
    assert_eq!(
        out,
        "G21 G90 G18 G54 G0 X0. Y0. Z0.\nG1 X10. Z0. F100.\nG2 X20. Z0. K0. I5.\n"
    );
}

#[test]
fn an_arc_in_another_plane_is_refused() {
    let src = format!("{HEAD}G1 X10. Y0. Z0. F100.\nG18 G2 X20. Z0. I5. K0.\nG17\n");
    let (line, why) = refused(&src, 2, 3, &rotate(XY, 90.0, [0.0, 0.0, 0.0]));
    assert_eq!(line, 3);
    assert!(why.contains("plane"), "{why}");
}

#[test]
fn a_full_turn_changes_nothing() {
    let src = format!("{HEAD}G1 X10. Y3. F100.\nG2 X20. Y3. I5.\nG91 G1 X1.\n");
    let r = run(&src, 2, 4, &rotate(XY, 360.0, [7.0, -2.0, 0.0])).expect("works");
    assert!(r.edits.is_empty(), "{:?}", r.edits);
}

#[test]
fn the_move_after_an_incremental_selection_is_adjusted() {
    let src = format!("{HEAD}G91 G1 X10. F100.\nX10.\nX10.\nG90 G0 Z50.\n");
    let (out, notes) = rotated(&src, 3, 3, &rotate(XY, 90.0, [10.0, 0.0, 0.0]));
    // Line 3 runs from X10 to X20; turned about X10 it ends at X10 Y10.
    assert_eq!(
        out,
        format!("{HEAD}G91 G1 X10. F100.\nX0. Y10.\nX20. Y-10.\nG90 G0 Z50.\n")
    );
    assert!(notes.iter().any(|n| n.line == 4), "{notes:?}");
}

#[test]
fn a_plane_that_is_not_two_linear_axes_is_refused() {
    let src = format!("{HEAD}G1 X10. F100.\n");
    for plane in [
        ArcPlane {
            first: 0,
            second: 0,
        },
        ArcPlane {
            first: 0,
            second: 7,
        },
    ] {
        assert!(
            run(&src, 2, 2, &rotate(plane, 90.0, [0.0; 3])).is_err(),
            "{plane:?}"
        );
    }
    assert!(run(&src, 2, 2, &rotate(XY, f64::NAN, [0.0; 3])).is_err());
    assert!(run(&src, 2, 2, &rotate(XY, 10.0, [f64::INFINITY, 0.0, 0.0])).is_err());
}

#[test]
fn an_arc_inside_the_selection_survives_rounding() {
    // Turned by 30°, the arc's start (the line before) is rounded to
    // 0.001 mm; the arc must still be rewritten, not refused.
    let src = format!("{HEAD}G1 X10. Y0. F100.\nG2 X20. Y0. I5. J0.\nG1 X30.\n");
    let (out, _) = rotated(&src, 2, 4, &rotate(XY, 30.0, [0.0, 0.0, 0.0]));
    assert_eq!(
        out,
        format!("{HEAD}G1 X8.66 Y5. F100.\nG2 X17.321 Y10. I4.33 J2.5\nG1 X25.981 Y15.\n")
    );
}
