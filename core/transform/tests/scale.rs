//! Scale (M4.8): one factor for the two axes of a plane (X and Y in the
//! app; Z is never scaled) about a centre. Arc centre offsets and R are
//! scaled with the moves; a zero or negative factor, and an arc in another
//! plane, are refused.

use dialect_fanuc::Fanuc;
use interpreter::Options;
use ir::{ArcPlane, Pos, MAX_AXES};
use machine_mill::Mill;
use transform::{apply_edits, transform, TransformError, Transformation, Transformed};

const XY: ArcPlane = ArcPlane {
    first: 0,
    second: 1,
};

fn scale(factor: f64, centre: [f64; 2]) -> Transformation {
    let mut c: Pos = [0.0; MAX_AXES];
    c[0] = centre[0];
    c[1] = centre[1];
    Transformation::Scale {
        plane: XY,
        factor,
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

fn scaled(src: &str, first: u32, last: u32, t: &Transformation) -> String {
    let result = run(src, first, last, t).unwrap_or_else(|e| panic!("refused: {e}"));
    String::from_utf8(apply_edits(src.as_bytes(), &result.edits).expect("edits apply"))
        .expect("utf-8")
}

fn refused(src: &str, first: u32, last: u32, t: &Transformation) -> (u32, String) {
    match run(src, first, last, t) {
        Err(TransformError::Refused { line, reason }) => (line, reason),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

const HEAD: &str = "G21 G90 G17 G54 G0 X0. Y0. Z5.\n";

#[test]
fn x_and_y_scale_about_the_centre_and_z_does_not() {
    let src = format!("{HEAD}G1 X10. Y20. Z-1. F100.\n");
    assert_eq!(
        scaled(&src, 2, 2, &scale(0.8, [0.0, 0.0])),
        format!("{HEAD}G1 X8. Y16. Z-1. F100.\n")
    );
    assert_eq!(
        scaled(&src, 2, 2, &scale(2.0, [10.0, 10.0])),
        format!("{HEAD}G1 X10. Y30. Z-1. F100.\n")
    );
}

#[test]
fn incremental_moves_are_scaled() {
    let src = format!("{HEAD}G1 X0. Y0. F100.\nG91 G1 X10. Y4.\nX10.\nG90 G0 Z50.\n");
    assert_eq!(
        scaled(&src, 2, 4, &scale(0.5, [0.0, 0.0])),
        format!("{HEAD}G1 X0. Y0. F100.\nG91 G1 X5. Y2.\nX5.\nG90 G0 Z50.\n")
    );
}

#[test]
fn centre_offsets_and_radii_scale() {
    let src = format!(
        "{HEAD}G1 X10. Y0. F100.\nG2 X20. Y0. I5. J0.\nG3 X30. Y0. R5.\nG2 X40. Y0. R-5.\n"
    );
    assert_eq!(
        scaled(&src, 2, 5, &scale(2.0, [0.0, 0.0])),
        format!(
            "{HEAD}G1 X20. Y0. F100.\nG2 X40. Y0. I10. J0.\nG3 X60. Y0. R10.\nG2 X80. Y0. R-10.\n"
        )
    );
}

#[test]
fn helices_keep_their_depth() {
    let src = format!("{HEAD}G1 X10. Y0. Z0. F100.\nG2 X10. Y0. Z-2. I-5. J0.\n");
    assert_eq!(
        scaled(&src, 2, 3, &scale(1.5, [0.0, 0.0])),
        format!("{HEAD}G1 X15. Y0. Z0. F100.\nG2 X15. Y0. Z-2. I-7.5 J0.\n")
    );
}

#[test]
fn rounding_goes_to_the_least_increment() {
    let src = format!("{HEAD}G1 X10. Y1. F100.\n");
    assert_eq!(
        scaled(&src, 2, 2, &scale(1.0 / 3.0, [0.0, 0.0])),
        format!("{HEAD}G1 X3.333 Y0.333 F100.\n")
    );
}

#[test]
fn a_zero_or_negative_factor_is_refused() {
    let src = format!("{HEAD}G1 X10. F100.\n");
    for f in [0.0, -1.0, -0.5, f64::NAN, f64::INFINITY] {
        let (line, why) = refused(&src, 2, 2, &scale(f, [0.0, 0.0]));
        assert_eq!(line, 2);
        assert!(why.contains("factor"), "{f}: {why}");
    }
}

#[test]
fn an_arc_in_another_plane_is_refused() {
    let src = format!("{HEAD}G1 X10. Y0. Z0. F100.\nG18 G2 X20. Z0. I5. K0.\nG17\n");
    let (line, why) = refused(&src, 2, 3, &scale(2.0, [0.0, 0.0]));
    assert_eq!(line, 3);
    assert!(why.contains("plane"), "{why}");
}

#[test]
fn a_factor_of_one_changes_nothing() {
    let src = format!("{HEAD}G1 X10. Y3. F100.\nG2 X20. Y3. I5.\nG3 X30. Y3. R5.\n");
    let r = run(&src, 2, 4, &scale(1.0, [7.0, -2.0])).expect("works");
    assert!(r.edits.is_empty(), "{:?}", r.edits);
}

#[test]
fn a_dialect_without_a_radius_letter_cannot_scale_r_arcs() {
    // Fanuc names its radius word; every dialect that rewrites must.
    use dialect_api::Dialect;
    assert_eq!(
        Fanuc::new().expect("dialect").arc_radius_letter(),
        Some('R')
    );
    assert_eq!(
        dialect_osai::Osai::new()
            .expect("dialect")
            .arc_radius_letter(),
        None
    );
}
