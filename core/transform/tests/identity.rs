//! The identity transformation (M4.4): rewriting every length and angle
//! word with its own value leaves any program unchanged, byte for byte,
//! with no edits at all. Checked on generated programs and on every golden
//! program, in mm and inches, with both of Fanuc's decimal-point settings.

use std::path::Path;

use dialect_api::{Dialect, ValueKind};
use dialect_fanuc::{DecimalInput, Fanuc, FanucOptions};
use gcode_syntax::{parse, Expr, NumberStyle};
use ir::LengthUnit;
use proptest::prelude::*;
use transform::{apply_edits, rewrite_word, source_lines, TextEdit};

/// What a word measures, for the letters that hold lengths or angles.
fn kind(letter: char) -> Option<ValueKind> {
    match letter {
        'X' | 'Y' | 'Z' | 'I' | 'J' | 'K' | 'R' | 'U' | 'V' | 'W' => Some(ValueKind::Length),
        'A' | 'B' | 'C' => Some(ValueKind::Angle),
        _ => None,
    }
}

/// The edits that rewrite every length and angle word of `source` with the
/// value it already has.
fn identity_edits(source: &[u8], dialect: &dyn Dialect, units: LengthUnit) -> Vec<TextEdit> {
    let lines = source_lines(source);
    let program = parse(source);
    let mut edits = Vec::new();
    for block in program.blocks.iter().filter(|b| b.errors.is_empty()) {
        let Some(line) = lines.get(block.line as usize - 1) else {
            continue;
        };
        for word in &block.words {
            let (Some(kind), Expr::Number(n)) = (kind(word.letter), &word.value) else {
                continue;
            };
            let value = dialect
                .read_value(n, kind, units)
                .expect("Fanuc reads numbers");
            // Stricter than "no edit": the dialect writes the value back as
            // exactly the same text, so the style is really kept. The one
            // exception is a zero written with a minus sign.
            let span = word.value_span(line).expect("a plain number");
            let old = &line[span.start..span.end];
            let style = NumberStyle::of(old).expect("a literal");
            let again = dialect
                .format_value(value, kind, units, &style)
                .expect("writable");
            if !(style.negative && value == 0.0) {
                assert_eq!(
                    again.as_bytes(),
                    old,
                    "line {}: {}",
                    block.line,
                    String::from_utf8_lossy(line)
                );
            }
            let edit = rewrite_word(line, block.line, word, value, kind, units, dialect)
                .expect("a plain number can be rewritten");
            edits.extend(edit);
        }
    }
    edits
}

fn dialects() -> Vec<Fanuc> {
    [DecimalInput::LeastIncrement, DecimalInput::Calculator]
        .into_iter()
        .map(|decimal_input| Fanuc::with_options(FanucOptions { decimal_input }).expect("dialect"))
        .collect()
}

fn check_identity(source: &[u8]) -> Result<(), TestCaseError> {
    for d in dialects() {
        for units in [LengthUnit::Metric, LengthUnit::Inch] {
            let edits = identity_edits(source, &d, units);
            prop_assert!(
                edits.is_empty(),
                "{:?} gave edits {:?}",
                String::from_utf8_lossy(source),
                edits
            );
            prop_assert_eq!(
                apply_edits(source, &edits).expect("applies"),
                source.to_vec()
            );
        }
    }
    Ok(())
}

#[test]
fn every_golden_program_is_unchanged() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("golden folder") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|x| x == "nc") {
            let source = std::fs::read(&path).expect("read");
            check_identity(&source).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            checked += 1;
        }
    }
    assert!(checked >= 5, "only {checked} golden programs found");
}

/// A literal in one of the styles programs use.
fn literal() -> impl Strategy<Value = String> {
    (
        prop_oneof![Just(""), Just("-"), Just("+"), Just("- ")],
        "[0-9]{0,5}",
        proptest::option::of("[0-9]{0,5}"),
    )
        .prop_filter_map("a literal needs a digit", |(sign, int, frac)| {
            let text = match frac {
                Some(f) => format!("{sign}{int}.{f}"),
                None => format!("{sign}{int}"),
            };
            text.bytes().any(|b| b.is_ascii_digit()).then_some(text)
        })
}

/// One item of a line: a word, a comment, or spacing.
fn item() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => ("[XYZIJKRABCxyz]", prop_oneof![Just(""), Just(" ")], literal())
            .prop_map(|(l, gap, v)| format!("{l}{gap}{v}")),
        2 => ("[GMFST]", "[0-9]{1,3}").prop_map(|(l, n)| format!("{l}{n}")),
        1 => "\\([a-zA-Z0-9 .=-]{0,12}\\)",
        1 => "[ \t]{1,2}",
    ]
}

/// A program of such lines, with N numbers, blank lines, `;` comments and
/// either line ending.
fn program() -> impl Strategy<Value = Vec<u8>> {
    (
        proptest::collection::vec(
            (
                proptest::option::of("N[0-9]{1,4} "),
                proptest::collection::vec(item(), 0..8),
                proptest::option::of(";[a-z ]{0,8}"),
            ),
            0..12,
        ),
        prop_oneof![Just("\n"), Just("\r\n")],
        any::<bool>(),
    )
        .prop_map(|(lines, ending, final_newline)| {
            let mut text = lines
                .into_iter()
                .map(|(n, items, comment)| {
                    format!(
                        "{}{}{}",
                        n.unwrap_or_default(),
                        items.concat(),
                        comment.unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join(ending);
            if final_newline {
                text.push_str(ending);
            }
            text.into_bytes()
        })
}

proptest! {
    #[test]
    fn any_program_is_unchanged(source in program()) {
        check_identity(&source)?;
    }
}
