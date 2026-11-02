//! Transformations on Osai 10 Series programs (M4.9a). Numbers are read
//! as written; in G90 the arc centre (I, J) is a position, in G91 a
//! distance from the start; I and J name the plane's first and second
//! axis whatever the plane (docs/dialect-notes/osai.md).

use dialect_osai::Osai;
use interpreter::Options;
use ir::{ArcPlane, Pos, MAX_AXES};
use machine_mill::Mill;
use transform::{apply_edits, transform, TransformError, Transformation, Transformed};

const XY: ArcPlane = ArcPlane {
    first: 0,
    second: 1,
};

fn pos(x: f64, y: f64, z: f64) -> Pos {
    let mut p: Pos = [0.0; MAX_AXES];
    p[0] = x;
    p[1] = y;
    p[2] = z;
    p
}

fn run(
    src: &str,
    first: u32,
    last: u32,
    t: &Transformation,
) -> Result<Transformed, TransformError> {
    let osai = Osai::new().expect("dialect");
    let mill = Mill::generic().expect("machine");
    transform(
        src.as_bytes(),
        &osai,
        &mill,
        Options::default(),
        first..=last,
        t,
    )
}

fn transformed(src: &str, first: u32, last: u32, t: &Transformation) -> String {
    let r = run(src, first, last, t).unwrap_or_else(|e| panic!("refused: {e}"));
    String::from_utf8(apply_edits(src.as_bytes(), &r.edits).expect("applies")).expect("utf-8")
}

fn refused(src: &str, first: u32, last: u32, t: &Transformation) -> (u32, String) {
    match run(src, first, last, t) {
        Err(TransformError::Refused { line, reason }) => (line, reason),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

const HEAD: &str = "G71 G90 G17\n(UAO,1)\nG0 X0 Y0 Z5\n";

#[test]
fn numbers_are_written_as_the_control_reads_them() {
    let src = format!("{HEAD}G1 X10 Y20.5 F100\n");
    let t = Transformation::Translate {
        offset: pos(2.5, 1.0, 0.0),
    };
    // X10 is 10 mm, so 12.5 needs a point; Y keeps its one decimal.
    assert_eq!(
        transformed(&src, 4, 4, &t),
        format!("{HEAD}G1 X12.5 Y21.5 F100\n")
    );
}

#[test]
fn an_absolute_centre_moves_with_a_translation() {
    // G90: I and J are the centre's position.
    let src = format!("{HEAD}G1 X10 Y0 F100\nG2 X20 Y0 I15 J0\nG1 X30\n");
    let t = Transformation::Translate {
        offset: pos(1.0, 2.0, 0.0),
    };
    assert_eq!(
        transformed(&src, 4, 5, &t),
        format!("{HEAD}G1 X11 Y2 F100\nG2 X21 Y2 I16 J2\nG1 X30\n")
    );
}

#[test]
fn an_absolute_centre_turns_about_the_rotation_centre() {
    let src = format!("{HEAD}G1 X10 Y0 F100\nG2 X20 Y0 I15 J0\n");
    let t = Transformation::Rotate {
        plane: XY,
        angle: 90.0,
        centre: pos(0.0, 0.0, 0.0),
    };
    assert_eq!(
        transformed(&src, 4, 5, &t),
        format!("{HEAD}G1 X0 Y10 F100\nG2 X0 Y20 I0 J15\n")
    );
}

#[test]
fn an_incremental_centre_is_a_distance_and_turns_like_one() {
    let src = format!("{HEAD}G1 X10 Y0 F100\nG91 G2 X10 Y0 I5 J0\nG90\n");
    let t = Transformation::Rotate {
        plane: XY,
        angle: 90.0,
        centre: pos(0.0, 0.0, 0.0),
    };
    assert_eq!(
        transformed(&src, 4, 5, &t),
        format!("{HEAD}G1 X0 Y10 F100\nG91 G2 X0 Y10 I0 J5\nG90\n")
    );
    // A missing J in G91 is a zero distance; turned, it is needed.
    let src = format!("{HEAD}G1 X10 Y0 F100\nG91 G2 X10 Y0 I5\nG90\n");
    assert_eq!(
        transformed(&src, 4, 5, &t),
        format!("{HEAD}G1 X0 Y10 F100\nG91 G2 X0 Y10 I0 J5\nG90\n")
    );
}

#[test]
fn an_absolute_centre_scales_about_the_scale_centre() {
    let src = format!("{HEAD}G1 X10 Y0 F100\nG2 X20 Y0 I15 J0\nG3 X30 Y0 R5\n");
    let t = Transformation::Scale {
        plane: XY,
        factor: 2.0,
        centre: pos(0.0, 0.0, 0.0),
    };
    assert_eq!(
        transformed(&src, 4, 6, &t),
        format!("{HEAD}G1 X20 Y0 F100\nG2 X40 Y0 I30 J0\nG3 X60 Y0 R10\n")
    );
}

#[test]
fn i_and_j_name_the_planes_axes_in_g18() {
    // G18 is the plane of the 3rd and 1st axes (Z, X): I is Z, J is X.
    let src = "G71 G90\nG18\n(UAO,1)\nG0 X0 Y0 Z0\nG1 X0 Z10 F100\nG2 X0 Z20 I15 J0\n";
    let t = Transformation::Translate {
        offset: pos(1.0, 0.0, 2.0),
    };
    assert_eq!(
        transformed(src, 5, 6, &t),
        "G71 G90\nG18\n(UAO,1)\nG0 X0 Y0 Z0\nG1 X1 Z12 F100\nG2 X1 Z22 I17 J1\n"
    );
}

#[test]
fn osai_codes_that_cannot_be_carried_over_are_refused() {
    for (block, word) in [
        ("(UAO,1,Z)", "UAO"),
        ("(ROT,30)", "ROT"),
        ("G92 X0 Y0", "G92"),
        ("G79 G0 Z0", "G79"),
    ] {
        let src = format!("{HEAD}G1 X10 F100\n{block}\nG1 X20\n");
        let (line, why) = refused(
            &src,
            4,
            6,
            &Transformation::Translate {
                offset: pos(1.0, 0.0, 0.0),
            },
        );
        assert_eq!(line, 5, "{block}: {why}");
        assert!(why.contains(word), "{block}: {why}");
    }
}

#[test]
fn identity_leaves_osai_programs_unchanged() {
    let src = format!(
        "{HEAD}G1 X10.5 Y-3 F100\nG2 X20.25 Y-3 I15.375 J-3\nG91 G3 X5 Y5 I0 J5\nG90 G1 X0\n"
    );
    let t = Transformation::Rotate {
        plane: XY,
        angle: 360.0,
        centre: pos(3.0, 4.0, 0.0),
    };
    assert!(run(&src, 4, 7, &t).expect("works").edits.is_empty());
}
