//! Tests for the shared helper functions.

use dialect_api::{length_to_internal, literal, report_syntax_errors, MM_PER_INCH};
use gcode_syntax::parse_str;
use ir::{DiagCode, Diagnostics, LengthUnit, Severity, SourceRef};

#[test]
fn unit_conversion() {
    assert_eq!(length_to_internal(12.5, LengthUnit::Metric), 12.5);
    assert_eq!(length_to_internal(2.0, LengthUnit::Inch), 50.8);
    assert_eq!(MM_PER_INCH, 25.4);
}

#[test]
fn literal_accepts_only_plain_numbers() {
    let block = &parse_str("X-1.5 Y#1 Z[1+2]").blocks[0];
    let n = literal(&block.words[0].value).unwrap();
    assert_eq!(n.value, -1.5);
    assert!(n.decimal_point);
    assert!(literal(&block.words[1].value).is_none());
    assert!(literal(&block.words[2].value).is_none());
}

#[test]
fn syntax_errors_become_diagnostics_with_columns() {
    let program = parse_str("G1 X10 @\n");
    let block = &program.blocks[0];
    let src = SourceRef {
        line: block.line,
        block: 0,
        ..SourceRef::default()
    };
    let mut diag = Diagnostics::new();
    assert!(report_syntax_errors(block, src, &mut diag));
    let d = &diag.as_slice()[0];
    assert_eq!(d.code, DiagCode::SyntaxError);
    assert_eq!(d.severity, Severity::Error);
    assert_eq!((d.src.line, d.src.col_start, d.src.col_end), (1, 7, 8));
    assert!(d.message.contains('@'));
}

#[test]
fn clean_block_reports_nothing() {
    let program = parse_str("G1 X10\n");
    let mut diag = Diagnostics::new();
    assert!(!report_syntax_errors(
        &program.blocks[0],
        SourceRef::default(),
        &mut diag
    ));
    assert!(diag.is_empty());
}

#[test]
fn huge_columns_saturate_instead_of_wrapping() {
    let line = format!("{}@", "G1 ".repeat(30_000));
    let program = parse_str(&line);
    let mut diag = Diagnostics::new();
    report_syntax_errors(&program.blocks[0], SourceRef::default(), &mut diag);
    assert_eq!(diag.as_slice()[0].src.col_start, u16::MAX);
}
