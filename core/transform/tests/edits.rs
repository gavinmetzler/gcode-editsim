//! Applying edits to the text by byte span (M4.4): every byte outside the
//! edits stays as it was.

use gcode_syntax::Span;
use transform::{apply_edits, source_lines, EditError, TextEdit};

fn edit(line: u32, start: usize, end: usize, text: &str) -> TextEdit {
    TextEdit {
        line,
        span: Span::new(start, end),
        text: text.to_owned(),
    }
}

fn apply(source: &str, edits: &[TextEdit]) -> String {
    String::from_utf8(apply_edits(source.as_bytes(), edits).expect("applies")).expect("utf-8")
}

#[test]
fn replaces_only_the_span() {
    let src = "G1 X10. Y20. (keep) F100.\n";
    assert_eq!(
        apply(src, &[edit(1, 4, 7, "12.5")]),
        "G1 X12.5 Y20. (keep) F100.\n"
    );
}

#[test]
fn no_edits_change_nothing() {
    let src = "G1 X10.\r\n\r\n(\u{fc}) Y2\n";
    assert_eq!(apply(src, &[]), src);
}

#[test]
fn several_edits_on_a_line_in_any_order() {
    let src = "G1 X1. Y2. Z3.\n";
    let edits = [
        edit(1, 12, 14, "30."),
        edit(1, 4, 6, "10."),
        edit(1, 8, 10, "20."),
    ];
    assert_eq!(apply(src, &edits), "G1 X10. Y20. Z30.\n");
}

#[test]
fn edits_on_several_lines() {
    let src = "X1.\nY2.\nZ3.";
    let edits = [edit(3, 1, 3, "33."), edit(1, 1, 3, "11.")];
    assert_eq!(apply(src, &edits), "X11.\nY2.\nZ33.");
}

#[test]
fn an_empty_span_inserts() {
    // Adding a word after the last axis word (M4.7 adds Y when rotating).
    let src = "G1 X10.\n";
    assert_eq!(apply(src, &[edit(1, 7, 7, " Y5.")]), "G1 X10. Y5.\n");
}

#[test]
fn keeps_windows_line_endings_and_other_bytes() {
    let src = b"G1 X1.\r\nY2. (\xff)\r\n".to_vec();
    let out = apply_edits(&src, &[edit(1, 4, 6, "7.5")]).expect("applies");
    assert_eq!(out, b"G1 X7.5\r\nY2. (\xff)\r\n".to_vec());
}

#[test]
fn lines_are_split_as_the_parser_splits_them() {
    let lines = source_lines(b"a\r\nb\n\nc");
    assert_eq!(lines, vec![&b"a"[..], b"b", b"", b"c"]);
}

#[test]
fn refuses_edits_that_do_not_fit() {
    let src = b"G1 X1.\r\nY2.\n";
    assert_eq!(
        apply_edits(src, &[edit(9, 0, 1, "")]),
        Err(EditError::NoSuchLine { line: 9 })
    );
    assert_eq!(
        apply_edits(src, &[edit(0, 0, 1, "")]),
        Err(EditError::NoSuchLine { line: 0 })
    );
    // The `\r` is not part of the line.
    assert!(matches!(
        apply_edits(src, &[edit(1, 4, 7, "")]),
        Err(EditError::OutsideLine { line: 1, .. })
    ));
    assert!(matches!(
        apply_edits(src, &[edit(1, 3, 2, "")]),
        Err(EditError::OutsideLine { line: 1, .. })
    ));
    assert_eq!(
        apply_edits(src, &[edit(1, 3, 5, "a"), edit(1, 4, 6, "b")]),
        Err(EditError::Overlapping { line: 1 })
    );
}

#[test]
fn edits_that_touch_are_not_overlapping() {
    let src = "X1.Y2.\n";
    let edits = [
        edit(1, 1, 3, "10."),
        edit(1, 3, 3, " "),
        edit(1, 4, 6, "20."),
    ];
    assert_eq!(apply(src, &edits), "X10. Y20.\n");
}
