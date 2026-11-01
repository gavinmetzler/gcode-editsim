//! Rewriting one word's number (M4.4): only the number changes, in the
//! program's style, and only if its value really changes.

use dialect_api::{Dialect, ValueKind};
use dialect_fanuc::Fanuc;
use gcode_syntax::parse;
use ir::LengthUnit;
use transform::{apply_edits, rewrite_word, RewriteError};

const MM: LengthUnit = LengthUnit::Metric;

/// `line` with word `index` rewritten to `value` mm.
fn rewritten(
    line: &str,
    index: usize,
    value: f64,
    dialect: &dyn Dialect,
) -> Result<String, RewriteError> {
    let program = parse(line.as_bytes());
    let word = &program.blocks[0].words[index];
    let edit = rewrite_word(
        line.as_bytes(),
        1,
        word,
        value,
        ValueKind::Length,
        MM,
        dialect,
    )?;
    let edits: Vec<_> = edit.into_iter().collect();
    Ok(String::from_utf8(apply_edits(line.as_bytes(), &edits).expect("applies")).expect("utf-8"))
}

#[test]
fn changes_only_the_number() {
    let d = Fanuc::new().expect("dialect");
    assert_eq!(
        rewritten("N10 G1 X10. Y20. (keep) F100.", 1, 12.5, &d).unwrap(),
        "N10 G1 X12.5 Y20. (keep) F100."
    );
    assert_eq!(
        rewritten("g1x10.y20.", 1, -3.0, &d).unwrap(),
        "g1x-3.y20.",
        "lower case and no spaces kept"
    );
    assert_eq!(
        rewritten("G1 X 10.", 1, 7.25, &d).unwrap(),
        "G1 X 7.25",
        "the space after the letter stays"
    );
}

#[test]
fn writes_as_the_controller_reads_it() {
    let d = Fanuc::new().expect("dialect");
    // X10 is 0.010 mm (DPI = 0), so 12.5 mm is 12500 increments.
    assert_eq!(rewritten("G1 X10", 1, 12.5, &d).unwrap(), "G1 X12500");
}

#[test]
fn the_same_value_is_not_an_edit() {
    let d = Fanuc::new().expect("dialect");
    let program = parse(b"G1 X- 010.500");
    let word = &program.blocks[0].words[1];
    let edit = rewrite_word(b"G1 X- 010.500", 1, word, -10.5, ValueKind::Length, MM, &d).unwrap();
    assert_eq!(edit, None);
    // Even a value that only differs below the least increment.
    let edit = rewrite_word(
        b"G1 X- 010.500",
        1,
        word,
        -10.5000001,
        ValueKind::Length,
        MM,
        &d,
    )
    .unwrap();
    assert_eq!(edit, None);
}

#[test]
fn refuses_a_word_that_is_not_a_plain_number() {
    let d = Fanuc::new().expect("dialect");
    assert_eq!(
        rewritten("G1 X#100", 1, 1.0, &d),
        Err(RewriteError::NotANumber {
            line: 1,
            letter: 'X'
        })
    );
    assert_eq!(
        rewritten("G1 X[1+2]", 1, 1.0, &d),
        Err(RewriteError::NotANumber {
            line: 1,
            letter: 'X'
        })
    );
}

#[test]
fn refuses_when_the_dialect_does_not_rewrite() {
    let osai = dialect_osai::Osai::new().expect("dialect");
    assert_eq!(
        rewritten("G1 X10.", 1, 1.0, &osai),
        Err(RewriteError::CannotWrite {
            line: 1,
            letter: 'X'
        })
    );
}

#[test]
fn refuses_a_value_that_cannot_be_written() {
    let d = Fanuc::new().expect("dialect");
    assert_eq!(
        rewritten("G1 X10.", 1, f64::NAN, &d),
        Err(RewriteError::CannotWrite {
            line: 1,
            letter: 'X'
        })
    );
}
