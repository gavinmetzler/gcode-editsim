//! Rewrites selected program lines exactly (M4: code transformations).
//!
//! The text is changed only where a value must change: each change is a
//! [`TextEdit`] (a line, a byte span in it, and the new text), and
//! [`apply_edits`] leaves every other byte as it was, comments, spacing,
//! word order and line endings included. [`rewrite_word`] makes the edit
//! for one word's new value, written by the dialect as its controller
//! reads it ([`Dialect::format_value`]) in the style of the number it
//! replaces; a value that reads back the same as before is no edit at all.
//!
//! Built so far (M4.4): edits and single-word rewrites. Working out the
//! new values of a selection (translate, rotate, scale) comes in later
//! steps (M4.5 to M4.8, `docs/plans/M4_PLAN.md`).
//!
//! Takes untrusted text and never panics: what cannot be done is an error.

#![forbid(unsafe_code)]

mod transformation;

pub use transformation::{transform, Note, TransformError, Transformation, Transformed};

use dialect_api::{Dialect, ValueKind};
use gcode_syntax::{Expr, NumberStyle, Span, Word};
use ir::LengthUnit;
use thiserror::Error;

/// A change to one line: the bytes `span` of line `line` replaced by
/// `text`. An empty span inserts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// 1-based line number.
    pub line: u32,
    /// Byte range within the line (without its line ending).
    pub span: Span,
    /// The new text.
    pub text: String,
}

/// Why edits could not be applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EditError {
    /// The program has no such line.
    #[error("line {line} is not in the program")]
    NoSuchLine {
        /// The line asked for.
        line: u32,
    },
    /// The span is not within the line.
    #[error("line {line}: bytes {}..{} are not within the line", span.start, span.end)]
    OutsideLine {
        /// The line.
        line: u32,
        /// The span asked for.
        span: Span,
    },
    /// Two edits change the same bytes.
    #[error("line {line}: two changes overlap")]
    Overlapping {
        /// The line.
        line: u32,
    },
}

/// Why a word could not be rewritten.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RewriteError {
    /// The word's value is not a plain number (a variable or expression).
    #[error("line {line}: {letter} is not a plain number, so it cannot be changed")]
    NotANumber {
        /// The line.
        line: u32,
        /// The word's letter.
        letter: char,
    },
    /// The dialect cannot write the value (it does not support rewriting,
    /// or the value is not a number).
    #[error("line {line}: the new {letter} value cannot be written for this control")]
    CannotWrite {
        /// The line.
        line: u32,
        /// The word's letter.
        letter: char,
    },
}

/// Where each line is in `source`, split as the parser splits it: at
/// `\n`, without a `\r` before it. Index 0 is line 1.
fn line_ranges(source: &[u8]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    loop {
        let end = source
            .get(start..)
            .and_then(|rest| rest.iter().position(|&b| b == b'\n'))
            .map_or(source.len(), |p| start + p);
        let content_end = if end > start && source.get(end - 1) == Some(&b'\r') {
            end - 1
        } else {
            end
        };
        ranges.push((start, content_end));
        if end >= source.len() {
            break;
        }
        start = end + 1;
    }
    ranges
}

/// The lines of `source` as the parser sees them (split at `\n`, without a
/// `\r` before it), so a block's `line` indexes them (`line - 1`) and its
/// spans are offsets into them.
pub fn source_lines(source: &[u8]) -> Vec<&[u8]> {
    line_ranges(source)
        .into_iter()
        .map(|(s, e)| source.get(s..e).unwrap_or_default())
        .collect()
}

/// `source` with `edits` applied. Every byte outside the edits, line
/// endings included, is kept as it was. Edits may come in any order; two
/// edits may touch but not overlap (two insertions at the same place go in
/// the order given). Nothing is applied if any edit is invalid.
pub fn apply_edits(source: &[u8], edits: &[TextEdit]) -> Result<Vec<u8>, EditError> {
    let ranges = line_ranges(source);
    // (absolute start, absolute end, order given, text)
    let mut placed = Vec::with_capacity(edits.len());
    for (order, e) in edits.iter().enumerate() {
        let index = (e.line as usize)
            .checked_sub(1)
            .ok_or(EditError::NoSuchLine { line: e.line })?;
        let &(start, end) = ranges
            .get(index)
            .ok_or(EditError::NoSuchLine { line: e.line })?;
        if e.span.start > e.span.end || e.span.end > end - start {
            return Err(EditError::OutsideLine {
                line: e.line,
                span: e.span,
            });
        }
        placed.push((start + e.span.start, start + e.span.end, order, e));
    }
    placed.sort_by_key(|&(s, e, order, _)| (s, e, order));
    for pair in placed.windows(2) {
        if let [a, b] = pair {
            if a.1 > b.0 {
                return Err(EditError::Overlapping { line: b.3.line });
            }
        }
    }
    let extra: usize = edits.iter().map(|e| e.text.len()).sum();
    let mut out = Vec::with_capacity(source.len() + extra);
    let mut at = 0;
    for (start, end, _, e) in placed {
        out.extend_from_slice(source.get(at..start).unwrap_or_default());
        out.extend_from_slice(e.text.as_bytes());
        at = end;
    }
    out.extend_from_slice(source.get(at..).unwrap_or_default());
    Ok(out)
}

/// The edit that gives `word` (parsed from `line_text`, line `line`) the
/// value `value` (mm or degrees, with the program in `units`), written by
/// `dialect` in the style of the number it replaces. Only the number
/// changes: the letter, any spaces and everything else stay. `None` if the
/// new text would read back as the value already there (so rewriting a
/// word with its own value never changes it).
pub fn rewrite_word(
    line_text: &[u8],
    line: u32,
    word: &Word,
    value: f64,
    kind: ValueKind,
    units: LengthUnit,
    dialect: &dyn Dialect,
) -> Result<Option<TextEdit>, RewriteError> {
    let letter = word.letter;
    let not_a_number = RewriteError::NotANumber { line, letter };
    let cannot = RewriteError::CannotWrite { line, letter };
    let Expr::Number(old) = &word.value else {
        return Err(not_a_number);
    };
    let span = word.value_span(line_text).ok_or(not_a_number.clone())?;
    let old_text = line_text
        .get(span.start..span.end)
        .ok_or(not_a_number.clone())?;
    let style = NumberStyle::of(old_text).ok_or(not_a_number)?;
    let old_value = dialect.read_value(old, kind, units).ok_or(cannot.clone())?;
    let text = dialect
        .format_value(value, kind, units, &style)
        .ok_or(cannot.clone())?;
    if text.as_bytes() == old_text {
        return Ok(None);
    }
    // What the new text reads as, through the same dialect.
    let reparsed = gcode_syntax::parse(format!("{letter}{text}").as_bytes());
    let new_value = match reparsed.blocks.first().and_then(|b| b.words.first()) {
        Some(Word {
            value: Expr::Number(n),
            ..
        }) => dialect.read_value(n, kind, units).ok_or(cannot.clone())?,
        _ => return Err(cannot),
    };
    if new_value == old_value {
        return Ok(None);
    }
    Ok(Some(TextEdit { line, span, text }))
}
