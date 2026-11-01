//! Reading and writing numbers as Fanuc does: the decimal-point rule
//! (parameter DPI), the least input increment, and inches. The interpreter
//! reads every length and angle through [`in_program_units`] and
//! [`to_internal`]; rewriting (M4.4) writes through [`format`], so the two
//! cannot disagree.

use dialect_api::{length_to_internal, ValueKind, MM_PER_INCH};
use gcode_syntax::{Number, NumberStyle};
use ir::LengthUnit;

use crate::{DecimalInput, FanucOptions};

/// The least input increment in program units, and its decimals: 0.001 mm
/// or degree, 0.0001 inch (IS-B, the common setting).
fn increment(kind: ValueKind, units: LengthUnit) -> (f64, usize) {
    match (kind, units) {
        (ValueKind::Angle, _) | (ValueKind::Length, LengthUnit::Metric) => (0.001, 3),
        (ValueKind::Length, LengthUnit::Inch) => (0.0001, 4),
    }
}

/// A literal's value in program units (mm, inch or degrees): without a
/// decimal point it counts least increments under DPI = 0.
pub(crate) fn in_program_units(
    options: FanucOptions,
    n: &Number,
    kind: ValueKind,
    units: LengthUnit,
) -> f64 {
    let scaled = !n.decimal_point && options.decimal_input == DecimalInput::LeastIncrement;
    if scaled {
        n.value * increment(kind, units).0
    } else {
        n.value
    }
}

/// A value in program units converted to internal units (mm, degrees).
pub(crate) fn to_internal(value: f64, kind: ValueKind, units: LengthUnit) -> f64 {
    match kind {
        ValueKind::Angle => value,
        ValueKind::Length => length_to_internal(value, units),
    }
}

/// `value` (mm or degrees) written so Fanuc reads it back within half an
/// increment, in `style`: with a decimal point if the literal had one;
/// otherwise in least increments (DPI = 0) or whole units (calculator
/// input, which needs a point after all when the value is not whole).
pub(crate) fn format(
    options: FanucOptions,
    value: f64,
    kind: ValueKind,
    units: LengthUnit,
    style: &NumberStyle,
) -> Option<String> {
    let in_units = match (kind, units) {
        (ValueKind::Length, LengthUnit::Inch) => value / MM_PER_INCH,
        _ => value,
    };
    if !in_units.is_finite() {
        return None;
    }
    let (step, decimals) = increment(kind, units);
    if style.decimal_point {
        return style.render_decimal(in_units, decimals);
    }
    let steps = (in_units / step).round();
    match options.decimal_input {
        DecimalInput::LeastIncrement => style.render_integer(steps),
        DecimalInput::Calculator => {
            let rounded = steps * step;
            if (rounded - rounded.round()).abs() < step / 2.0 {
                style.render_integer(rounded.round())
            } else {
                let with_point = NumberStyle {
                    decimal_point: true,
                    ..style.clone()
                };
                with_point.render_decimal(in_units, decimals)
            }
        }
    }
}
