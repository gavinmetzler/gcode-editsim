//! Tests for `Diagnostic` and the `Diagnostics` collector.

use std::collections::HashSet;

use ir::{DiagCode, Diagnostic, Diagnostics, Severity, SourceRef};

fn at(line: u32) -> SourceRef {
    SourceRef {
        line,
        ..SourceRef::default()
    }
}

#[test]
fn collector_keeps_report_order_and_counts() {
    let mut d = Diagnostics::new();
    assert!(d.is_empty());
    assert!(!d.has_errors());

    d.warning(
        DiagCode::ControllerDependent,
        "assumed common behaviour",
        at(3),
    );
    d.info(DiagCode::NotYetSimulated, "noted", at(4));
    assert!(!d.has_errors());
    d.error(DiagCode::UnsupportedGCode, "G999 is not supported", at(1));
    d.push(Diagnostic::warning(DiagCode::InvalidValue, "odd", at(2)).dialect_specific());

    assert_eq!(d.len(), 4);
    assert!(d.has_errors());
    assert_eq!(d.count(Severity::Warning), 2);
    assert_eq!(d.count(Severity::Error), 1);
    assert_eq!(d.count(Severity::Info), 1);

    let lines: Vec<u32> = d.iter().map(|x| x.src.line).collect();
    assert_eq!(lines, vec![3, 4, 1, 2]);
    let specific: Vec<bool> = d.iter().map(|x| x.dialect_specific).collect();
    assert_eq!(specific, vec![false, false, false, true]);
}

#[test]
fn collector_converts_and_extends() {
    let mut a = Diagnostics::new();
    a.error(DiagCode::SyntaxError, "bad", at(1));
    let mut b = Diagnostics::new();
    b.extend(a.clone());
    b.extend([Diagnostic::info(DiagCode::NotYetSimulated, "x", at(2))]);
    assert_eq!(b.as_slice().len(), 2);
    assert_eq!((&b).into_iter().count(), 2);
    let v: Vec<Diagnostic> = b.into();
    assert_eq!(v[0], a.into_vec()[0]);
}

#[test]
fn diagnostic_constructors() {
    let d = Diagnostic::new(Severity::Warning, DiagCode::ArcRadiusMismatch, "r", at(9));
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.code, DiagCode::ArcRadiusMismatch);
    assert_eq!(d.message, "r");
    assert_eq!(d.src.line, 9);
    assert!(!d.dialect_specific);
}

#[test]
fn severity_is_ordered_by_seriousness() {
    assert!(Severity::Info < Severity::Warning);
    assert!(Severity::Warning < Severity::Error);
}

#[test]
fn diag_code_ids_are_stable_and_unique() {
    // These numbers go into the render buffer. Changing one is a breaking
    // format change: add new codes with new numbers instead.
    let codes = [
        (DiagCode::SyntaxError, 1),
        (DiagCode::UnsupportedGCode, 2),
        (DiagCode::UnsupportedMCode, 3),
        (DiagCode::UnsupportedWord, 4),
        (DiagCode::ModalGroupConflict, 5),
        (DiagCode::MissingWord, 6),
        (DiagCode::MissingFeedRate, 7),
        (DiagCode::ArcRadiusMismatch, 8),
        (DiagCode::InvalidArc, 9),
        (DiagCode::InvalidValue, 10),
        (DiagCode::OutOfLimits, 11),
        (DiagCode::ControllerDependent, 12),
        (DiagCode::NotYetSimulated, 13),
    ];
    let mut seen = HashSet::new();
    for (code, id) in codes {
        assert_eq!(code.id(), id, "{code:?}");
        assert!(seen.insert(id));
    }
}
