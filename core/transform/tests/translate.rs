//! Translate (M4.6): the selected lines are moved by an offset; only the
//! words that must change are rewritten, and the rest of the program is
//! unchanged (M4_PLAN §3).

use dialect_fanuc::Fanuc;
use interpreter::Options;
use ir::{Pos, Severity, MAX_AXES};
use machine_mill::Mill;
use transform::{apply_edits, transform, Note, TransformError, Transformation, Transformed};

fn offset(x: f64, y: f64, z: f64) -> Transformation {
    let mut p: Pos = [0.0; MAX_AXES];
    p[0] = x;
    p[1] = y;
    p[2] = z;
    Transformation::Translate { offset: p }
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

/// The program after translating lines `first..=last`.
fn translated(src: &str, first: u32, last: u32, t: &Transformation) -> (String, Vec<Note>) {
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
fn absolute_moves_get_the_offset() {
    let src = format!("{HEAD}G1 X10. Y10. F100.\nX20.\nG0 Z5.\n");
    let (out, _) = translated(&src, 2, 3, &offset(5.0, 0.0, 0.0));
    assert_eq!(out, format!("{HEAD}G1 X15. Y10. F100.\nX25.\nG0 Z5.\n"));
}

#[test]
fn an_axis_that_now_changes_gets_a_word() {
    let src = format!("{HEAD}G1 X10. Y10. F100.\nX20.\nG0 Z5.\n");
    // Only line 3 is selected: its Y (10, from line 2) must become 13.
    let (out, _) = translated(&src, 3, 3, &offset(0.0, 3.0, 0.0));
    assert_eq!(
        out,
        format!("{HEAD}G1 X10. Y10. F100.\nX20. Y13.\nG0 Z5.\n")
    );
    // With line 2 selected too, line 3 inherits Y13 and needs no word.
    let (out, _) = translated(&src, 2, 3, &offset(0.0, 3.0, 0.0));
    assert_eq!(out, format!("{HEAD}G1 X10. Y13. F100.\nX20.\nG0 Z5.\n"));
}

#[test]
fn comments_spacing_and_number_style_are_kept() {
    let src = format!("{HEAD}N20 g1 x10.000 (cut it) Y-02.5 f100.\n");
    let (out, _) = translated(&src, 2, 2, &offset(1.0, -1.0, 0.0));
    assert_eq!(out, format!("{HEAD}N20 g1 x11.000 (cut it) Y-03.5 f100.\n"));
}

#[test]
fn numbers_without_a_point_stay_without_one() {
    let src = "G21 G90 G54 G0 X0 Y0\nG1 X10000 F100.\n";
    let (out, _) = translated(src, 2, 2, &offset(2.5, 0.0, 0.0));
    assert_eq!(out, "G21 G90 G54 G0 X0 Y0\nG1 X12500 F100.\n");
}

#[test]
fn inch_sections_stay_in_inches() {
    let src = "G20 G90 G17 G54 G0 X0. Y0.\nG1 X1. F10.\n";
    let (out, _) = translated(src, 2, 2, &offset(25.4, 0.0, 0.0));
    assert_eq!(out, "G20 G90 G17 G54 G0 X0. Y0.\nG1 X2. F10.\n");
}

#[test]
fn incremental_moves_change_only_at_the_edges() {
    // Lines 3 and 4 are incremental; line 3 is selected. It gains the
    // offset, and line 4, the first move after the selection, loses it, so
    // the rest of the program is where it was.
    let src = format!("{HEAD}G91 G1 X10. F100.\nX10.\nX10.\nG90 G0 Z50.\n");
    let (out, notes) = translated(&src, 3, 3, &offset(5.0, 2.0, 0.0));
    assert_eq!(
        out,
        format!("{HEAD}G91 G1 X10. F100.\nX15. Y2.\nX5. Y-2.\nG90 G0 Z50.\n")
    );
    assert!(
        notes
            .iter()
            .any(|n| n.line == 4 && n.message.contains("after the selection")),
        "{notes:?}"
    );
}

#[test]
fn moves_inside_an_incremental_selection_are_unchanged() {
    let src = format!("{HEAD}G91 G1 X10. F100.\nX10.\nY10.\nX-10.\nG90 G0 Z50.\n");
    let (out, _) = translated(&src, 3, 5, &offset(5.0, 0.0, 0.0));
    assert_eq!(
        out,
        format!("{HEAD}G91 G1 X10. F100.\nX15.\nY10.\nX-10.\nG90 G0 Z50.\n")
    );
}

#[test]
fn arcs_keep_their_centre_offsets_and_radius() {
    let src = format!("{HEAD}G1 X10. Y0. F100.\nG2 X20. Y0. I5. J0.\nG3 X30. Y0. R5.\nG1 X40.\n");
    let (out, notes) = translated(&src, 2, 4, &offset(0.0, 5.0, 0.0));
    assert_eq!(
        out,
        format!("{HEAD}G1 X10. Y5. F100.\nG2 X20. Y5. I5. J0.\nG3 X30. Y5. R5.\nG1 X40.\n")
    );
    // Line 5 (absolute) is not edited: it now starts from the moved point.
    assert!(notes.iter().any(|n| n.line == 5), "{notes:?}");
}

#[test]
fn helices_move_along_their_axis_too() {
    let src = format!("{HEAD}G1 X10. Y0. Z0. F100.\nG2 X10. Y0. Z-2. I5. J0.\nG1 X0.\n");
    let (out, _) = translated(&src, 2, 3, &offset(0.0, 0.0, -1.0));
    assert_eq!(
        out,
        format!("{HEAD}G1 X10. Y0. Z-1. F100.\nG2 X10. Y0. Z-3. I5. J0.\nG1 X0.\n")
    );
}

#[test]
fn an_arc_cannot_start_or_follow_the_selection() {
    let src = format!("{HEAD}G1 X10. Y0. F100.\nG2 X20. Y0. I5. J0.\nG1 X30.\nG2 X40. Y0. R5.\n");
    let (line, why) = refused(&src, 3, 4, &offset(0.0, 5.0, 0.0));
    assert_eq!(line, 3);
    assert!(why.contains("arc"), "{why}");
    let (line, why) = refused(&src, 2, 4, &offset(0.0, 5.0, 0.0));
    assert_eq!(line, 5);
    assert!(
        why.contains("arc") && why.contains("after the selection"),
        "{why}"
    );
}

#[test]
fn codes_that_cannot_be_carried_over_are_refused() {
    for (block, word) in [
        ("G53 G0 Z0.", "G53"),
        ("G91 G28 Z0.", "G28"),
        ("G81 X5. Y5. Z-2. R1. F100.", "G81"),
        ("G92 X0. Y0.", "G92"),
        ("G1 X#100", "variable"),
    ] {
        let src = format!("{HEAD}G1 X10. F100.\n{block}\nG90 G1 X20.\n");
        let (line, why) = refused(&src, 2, 4, &offset(1.0, 0.0, 0.0));
        assert_eq!(line, 3, "{block}: {why}");
        assert!(
            why.contains(word) || why.contains("error"),
            "{block}: {why}"
        );
    }
}

#[test]
fn an_unknown_position_is_refused() {
    let src = format!("{HEAD}G91 G30 Z0.\nG90 G0 X5.\n");
    let (line, _) = refused(&src, 3, 3, &offset(1.0, 0.0, 0.0));
    assert_eq!(line, 3);
}

#[test]
fn a_dialect_that_cannot_rewrite_is_refused() {
    let osai = dialect_osai::Osai::new().expect("dialect");
    let mill = Mill::generic().expect("machine");
    let r = transform(
        b"G71\nX10 Y20\n",
        &osai,
        &mill,
        Options::default(),
        1..=2,
        &offset(1.0, 0.0, 0.0),
    );
    assert!(matches!(r, Err(TransformError::Refused { .. })), "{r:?}");
}

#[test]
fn an_offset_on_an_axis_the_machine_lacks_is_refused() {
    let mut p: Pos = [0.0; MAX_AXES];
    p[5] = 1.0;
    let src = format!("{HEAD}G1 X10. F100.\n");
    assert!(matches!(
        run(&src, 2, 2, &Transformation::Translate { offset: p }),
        Err(TransformError::Refused { .. })
    ));
    p[5] = f64::NAN;
    assert!(run(&src, 2, 2, &Transformation::Translate { offset: p }).is_err());
}

#[test]
fn no_offset_changes_nothing() {
    let src = format!("{HEAD}G91 G1 X10. F100.\nX10.\nG90 G2 X30. Y0. R5.\n");
    let r = run(&src, 1, 4, &offset(0.0, 0.0, 0.0)).expect("works");
    assert!(r.edits.is_empty(), "{:?}", r.edits);
}

#[test]
fn subprogram_lines_change_every_call_and_say_so() {
    let src = "G21 G90 G17 G54 G0 X0. Y0. Z5.\n\
               M98 P10 L2\n\
               M30\n\
               O10\n\
               G91 G0 X10.\n\
               G90 G1 Y10. F100.\n\
               M99\n";
    let (out, notes) = translated(src, 6, 6, &offset(0.0, 1.0, 0.0));
    assert!(out.contains("G90 G1 Y11. F100."), "{out}");
    assert!(
        notes
            .iter()
            .any(|n| n.severity == Severity::Warning && n.message.contains("2 times")),
        "{notes:?}"
    );
}

#[test]
fn a_called_subprogram_is_not_changed_and_that_is_said() {
    let src = "G21 G90 G17 G54 G0 X0. Y0. Z5.\n\
               G1 X10. F100.\n\
               M98 P10\n\
               G1 X20.\n\
               M30\n\
               O10\n\
               G1 Y5.\n\
               M99\n";
    let (out, notes) = translated(src, 2, 4, &offset(1.0, 0.0, 0.0));
    assert!(
        out.contains("G1 X11. F100.") && out.contains("G1 X21."),
        "{out}"
    );
    assert!(
        out.contains("G1 Y5.\n"),
        "the subprogram is unchanged: {out}"
    );
    assert!(
        notes
            .iter()
            .any(|n| n.line == 3 && n.severity == Severity::Warning),
        "{notes:?}"
    );
}

#[test]
fn selected_lines_that_never_run_are_left_and_noted() {
    let src = format!("{HEAD}G1 X10. F100.\nM30\nG1 X99.\n");
    let (out, notes) = translated(&src, 2, 4, &offset(1.0, 0.0, 0.0));
    assert!(out.ends_with("G1 X99.\n"), "{out}");
    assert!(notes.iter().any(|n| n.line == 4), "{notes:?}");
}

#[test]
fn the_connecting_moves_are_named() {
    let src = format!("{HEAD}G1 X10. F100.\nX20.\nX30.\n");
    let (_, notes) = translated(&src, 3, 3, &offset(0.0, 1.0, 0.0));
    assert!(
        notes
            .iter()
            .any(|n| n.line == 3 && n.severity == Severity::Info),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|n| n.line == 4 && n.severity == Severity::Info),
        "{notes:?}"
    );
}
