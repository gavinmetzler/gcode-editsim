//! Helpers shared by the interpreter tests.

#![allow(dead_code)]

use dialect_fanuc::Fanuc;
use interpreter::{interpret, Options};
use ir::{DiagCode, Pos, Program, SegKind, Segment, Severity, MAX_AXES};
use machine_mill::Mill;

/// Runs `src` with the Fanuc dialect on the shipped generic 3-axis mill:
/// G54 is at machine X-380 Y-205 Z-400, G55 at X-200 Y-205 Z-400, travel
/// X -760..0, Y -410..0, Z -510..0.
pub fn run(src: &str) -> Program {
    run_opts(src, Options::default())
}

/// Like [`run`], with options.
///
/// Most tests leave units, plane, distance and motion mode unset to stay
/// short. The dialect's "assumed ..." warnings for those are removed here;
/// `dialect-fanuc/tests/assumed_defaults.rs` and the golden files cover
/// them. Use [`run_unfiltered`] to keep them.
pub fn run_opts(src: &str, options: Options) -> Program {
    let mut p = run_unfiltered(src, options);
    p.diagnostics.retain(|d| {
        !(d.code == DiagCode::ControllerDependent && d.message.starts_with("assumed "))
    });
    p
}

/// Runs without removing any diagnostics.
pub fn run_unfiltered(src: &str, options: Options) -> Program {
    let fanuc = Fanuc::new().expect("dialect loads");
    let mill = Mill::generic().expect("machine loads");
    interpret(src.as_bytes(), "test.nc", &fanuc, &mill, options)
}

pub fn segments(p: &Program) -> &[Segment] {
    &p.channels[0].segments
}

pub fn moves(p: &Program) -> Vec<&Segment> {
    segments(p).iter().filter(|s| s.kind.is_motion()).collect()
}

pub fn codes(p: &Program) -> Vec<DiagCode> {
    p.diagnostics.iter().map(|d| d.code).collect()
}

pub fn errors(p: &Program) -> Vec<DiagCode> {
    p.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code)
        .collect()
}

pub fn pos(values: &[f64]) -> Pos {
    let mut p = [0.0; MAX_AXES];
    p[..values.len()].copy_from_slice(values);
    p
}

/// Asserts the first three axes of `p` are within 1e-9 of `expected`.
pub fn at(p: &Pos, expected: [f64; 3]) {
    for (axis, (a, b)) in p.iter().zip(expected).enumerate() {
        assert!((a - b).abs() < 1e-9, "axis {axis}: {a} != {b} in {p:?}");
    }
}

pub fn kinds(p: &Program) -> Vec<SegKind> {
    segments(p).iter().map(|s| s.kind).collect()
}
