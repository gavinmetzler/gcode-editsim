//! Reading and writing numbers as the 10 Series does (M4.9a): a number is
//! read as written, with or without a decimal point (manual p. 2-4), up to
//! 9 decimals (p. 1-15). The manual gives no input increment (the
//! resolution is a machine setting), so rewritten values are rounded to
//! 0.001 mm or degree and 0.0001 inch, or to the decimals the number
//! already had if it had more: our choice, noted in
//! `docs/dialect-notes/osai.md`.

use dialect_api::{format_as_written, length_to_internal, ValueKind, MM_PER_INCH};
use gcode_syntax::{Number, NumberStyle};
use ir::LengthUnit;

/// Decimals a rewritten value is rounded to (at least).
fn decimals(kind: ValueKind, units: LengthUnit) -> usize {
    match (kind, units) {
        (ValueKind::Length, LengthUnit::Inch) => 4,
        _ => 3,
    }
}

/// A literal in internal units (mm, degrees).
pub(crate) fn read(n: &Number, kind: ValueKind, units: LengthUnit) -> f64 {
    match kind {
        ValueKind::Angle => n.value,
        ValueKind::Length => length_to_internal(n.value, units),
    }
}

/// `value` (mm or degrees) written as the control reads it, in `style`.
pub(crate) fn format(
    value: f64,
    kind: ValueKind,
    units: LengthUnit,
    style: &NumberStyle,
) -> Option<String> {
    let in_units = match (kind, units) {
        (ValueKind::Length, LengthUnit::Inch) => value / MM_PER_INCH,
        _ => value,
    };
    format_as_written(in_units, style, decimals(kind, units))
}
