//! JSON round trip of a whole program. Needs `--features serde`.

#![cfg(feature = "serde")]

mod common;

use common::{arc, line_seg};
use ir::{
    bounding_box, ChannelStream, DiagCode, Diagnostic, LengthUnit, Program, ProgramMeta, SegFlags,
    SegKind, SourceRef,
};

#[test]
fn program_round_trips_through_json() {
    let mut cut = line_seg(&[0.0, 0.0], &[10.0, 0.0]);
    cut.flags = SegFlags::COMP_ACTIVE;
    let segments = vec![
        cut,
        arc(
            SegKind::ArcCcw,
            &[10.0, 0.0],
            &[-10.0, 0.0],
            &[0.0, 0.0],
            10.0,
        ),
    ];
    let program = Program {
        meta: ProgramMeta {
            units_used: vec![LengthUnit::Metric],
            tools_used: vec![1],
            bbox: bounding_box(&segments),
            estimated_time: None,
        },
        channels: vec![ChannelStream {
            channel: 0,
            segments,
        }],
        diagnostics: vec![Diagnostic::warning(
            DiagCode::ControllerDependent,
            "check this",
            SourceRef::default(),
        )],
        files: vec!["main.nc".into()],
    };

    let json = serde_json::to_string(&program).expect("serialize");
    // Readable, stable names for enums; flags as a plain number.
    assert!(json.contains("\"ArcCcw\""));
    assert!(json.contains("\"ControllerDependent\""));
    assert!(json.contains("\"flags\":1"));

    let back: Program = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, program);
}
