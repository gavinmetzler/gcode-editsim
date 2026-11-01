//! Transformations as the app asks for them (M4.9): small JSON in, the
//! edits and notes out as a compact binary list, and program coordinates
//! for a point picked in the view.

use app_backend::files::shipped_machine;
use app_backend::transform::{
    decode_edits, encode_edits, program_point, transform_program, EditList, TransformRequest,
    EDITS_MAGIC, EDITS_VERSION,
};
use app_backend::{BackendError, RunOptions};
use ir::Severity;
use transform::{apply_edits, Note, TextEdit, Transformed};

fn mill() -> &'static str {
    shipped_machine("generic_3axis_mill").expect("shipped")
}

fn request(json: &str) -> TransformRequest {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

fn run(program: &str, json: &str) -> Result<String, BackendError> {
    let bytes = transform_program(
        program.as_bytes(),
        mill(),
        "fanuc",
        RunOptions::default(),
        &request(json),
    )?;
    let list = decode_edits(&bytes).expect("decodes");
    let out = apply_edits(program.as_bytes(), &list.edits).expect("applies");
    Ok(String::from_utf8(out).expect("utf-8"))
}

const PROGRAM: &str =
    "G21 G90 G17 G54 G0 X0. Y0. Z5.\nG1 X10. Y0. F100.\nG2 X20. Y0. I5. J0.\nG1 X30.\n";

#[test]
fn translate_by_axis_letters() {
    let out = run(PROGRAM, r#"{"first": 2, "last": 3, "transform": {"kind": "translate", "offset": {"X": 1, "y": 2}}}"#).unwrap();
    assert_eq!(
        out,
        "G21 G90 G17 G54 G0 X0. Y0. Z5.\nG1 X11. Y2. F100.\nG2 X21. Y2. I5. J0.\nG1 X30.\n"
    );
}

#[test]
fn rotate_in_a_plane_named_by_letters() {
    let out = run(
        PROGRAM,
        r#"{"first": 2, "last": 3, "transform": {"kind": "rotate", "plane": ["X", "Y"], "angle": 90, "centre": {"X": 0, "Y": 0}}}"#,
    )
    .unwrap();
    assert_eq!(
        out,
        "G21 G90 G17 G54 G0 X0. Y0. Z5.\nG1 X0. Y10. F100.\nG2 X0. Y20. I0. J5.\nG1 X30.\n"
    );
}

#[test]
fn scale_in_a_plane_named_by_letters() {
    let out = run(
        PROGRAM,
        r#"{"first": 2, "last": 3, "transform": {"kind": "scale", "plane": ["X", "Y"], "factor": 2, "centre": {}}}"#,
    )
    .unwrap();
    assert_eq!(
        out,
        "G21 G90 G17 G54 G0 X0. Y0. Z5.\nG1 X20. Y0. F100.\nG2 X40. Y0. I10. J0.\nG1 X30.\n"
    );
}

#[test]
fn a_refusal_is_an_error_naming_the_line() {
    let err = run(
        "G21 G90 G54 G0 X0. Y0. Z5.\nG53 G0 Z0.\n",
        r#"{"first": 2, "last": 2, "transform": {"kind": "translate", "offset": {"X": 1}}}"#,
    )
    .unwrap_err();
    let text = err.to_string();
    assert!(matches!(err, BackendError::TransformRefused(_)), "{err:?}");
    assert!(text.contains("line 2") && text.contains("G53"), "{text}");
}

#[test]
fn an_axis_the_machine_lacks_is_an_error() {
    for json in [
        r#"{"first": 2, "last": 2, "transform": {"kind": "translate", "offset": {"A": 1}}}"#,
        r#"{"first": 2, "last": 2, "transform": {"kind": "translate", "offset": {"XY": 1}}}"#,
        r#"{"first": 2, "last": 2, "transform": {"kind": "rotate", "plane": ["X", "Q"], "angle": 9, "centre": {}}}"#,
    ] {
        let err = run(PROGRAM, json).unwrap_err();
        assert!(
            matches!(err, BackendError::InvalidRequest(_)),
            "{json}: {err:?}"
        );
    }
    // A zero offset on an unknown axis does nothing, so it is no error.
    assert!(run(
        PROGRAM,
        r#"{"first": 2, "last": 2, "transform": {"kind": "translate", "offset": {"A": 0}}}"#
    )
    .is_ok());
}

#[test]
fn unknown_request_fields_are_refused() {
    for json in [
        r#"{"first": 1, "last": 1, "transform": {"kind": "mirror", "axis": "X"}}"#,
        r#"{"first": 1, "last": 1, "transform": {"kind": "translate", "offset": {}, "extra": 1}}"#,
        r#"{"first": 1, "last": 1, "extra": 1, "transform": {"kind": "translate", "offset": {}}}"#,
    ] {
        assert!(
            serde_json::from_str::<TransformRequest>(json).is_err(),
            "{json}"
        );
    }
    let good = r#"{"first": 1, "last": 1, "transform": {"kind": "translate", "offset": {}}}"#;
    assert!(serde_json::from_str::<TransformRequest>(good).is_ok());
}

#[test]
fn the_edit_list_round_trips() {
    let t = Transformed {
        edits: vec![
            TextEdit {
                line: 3,
                span: gcode_syntax::Span::new(4, 7),
                text: "12.5".to_owned(),
            },
            TextEdit {
                line: 9,
                span: gcode_syntax::Span::new(0, 0),
                text: " Y\u{fc}".to_owned(),
            },
        ],
        notes: vec![Note {
            severity: Severity::Warning,
            line: 4,
            message: "check line 4".to_owned(),
        }],
    };
    let bytes = encode_edits(&t);
    assert_eq!(&bytes[0..4], &EDITS_MAGIC.to_le_bytes());
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), EDITS_VERSION);
    assert_eq!(bytes.len() % 4, 0);
    let back = decode_edits(&bytes).expect("decodes");
    assert_eq!(
        back,
        EditList {
            edits: t.edits.clone(),
            notes: t.notes.clone()
        }
    );
}

#[test]
fn a_damaged_edit_list_is_an_error_not_a_panic() {
    let good = encode_edits(&Transformed {
        edits: vec![TextEdit {
            line: 1,
            span: gcode_syntax::Span::new(0, 1),
            text: "x".to_owned(),
        }],
        notes: vec![],
    });
    for cut in 0..good.len() {
        assert!(decode_edits(&good[..cut]).is_none(), "cut at {cut}");
    }
    let mut bad = good.clone();
    bad[0] ^= 1;
    assert!(decode_edits(&bad).is_none());
}

/// The fixture the UI's decoder test reads (`ui/tests/edits.test.ts`). It
/// must match what this code writes; regenerate with `UPDATE_GOLDEN=1`
/// after a deliberate layout change (and bump the version).
#[test]
fn the_shared_fixture_matches() {
    let t = Transformed {
        edits: vec![
            TextEdit {
                line: 2,
                span: gcode_syntax::Span::new(4, 7),
                text: "11.".to_owned(),
            },
            TextEdit {
                line: 2,
                span: gcode_syntax::Span::new(11, 11),
                text: " Y2.".to_owned(),
            },
            TextEdit {
                line: 5,
                span: gcode_syntax::Span::new(9, 12),
                text: "-\u{e9}5".to_owned(),
            },
        ],
        notes: vec![
            Note {
                severity: Severity::Info,
                line: 2,
                message: "first move".to_owned(),
            },
            Note {
                severity: Severity::Warning,
                line: 6,
                message: "after the selection".to_owned(),
            },
        ],
    };
    let bytes = encode_edits(&t);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/transform_edits_v1.bin");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &bytes).expect("write fixture");
    }
    let fixture = std::fs::read(&path).expect("fixture (UPDATE_GOLDEN=1 writes it)");
    assert_eq!(fixture, bytes);
}

#[test]
fn a_picked_point_in_program_coordinates() {
    // G54 on the generic mill is at machine X-380 Y-205 Z-400, G55 at X-200.
    let program = b"G21 G90 G54 G0 X0. Y0. Z5.\nG55 G0 X1.\n";
    let p = program_point(
        program,
        mill(),
        "fanuc",
        RunOptions::default(),
        1,
        [-370.0, -195.0, -395.0],
    )
    .unwrap();
    assert_eq!(p, vec![('X', 10.0), ('Y', 10.0), ('Z', 5.0)]);
    let p = program_point(
        program,
        mill(),
        "fanuc",
        RunOptions::default(),
        2,
        [-190.0, -205.0, -395.0],
    )
    .unwrap();
    assert_eq!(p[0], ('X', 10.0));
    assert!(program_point(program, mill(), "fanuc", RunOptions::default(), 9, [0.0; 3]).is_err());
}
