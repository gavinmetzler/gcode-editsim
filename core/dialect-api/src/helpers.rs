//! Small building blocks shared by dialect implementations.

use gcode_syntax::{Block, Expr, Number, NumberStyle};
use ir::{DiagCode, Diagnostic, Diagnostics, LengthUnit, SourceRef};

/// Millimetres per inch.
pub const MM_PER_INCH: f64 = 25.4;

/// Converts a length in `unit` to internal units (mm).
pub fn length_to_internal(value: f64, unit: LengthUnit) -> f64 {
    match unit {
        LengthUnit::Metric => value,
        LengthUnit::Inch => value * MM_PER_INCH,
    }
}

/// `value` (in program units) written for a control that reads numbers as
/// written, with or without a decimal point (calculator-style input): in
/// the style of the literal it replaces, rounded to `max_decimals` (at
/// least the decimals the literal had). A literal without a point stays
/// without one when the rounded value is whole, and gains a point when it
/// is not, since the whole number would mean something else. `None` if the
/// value is not finite.
pub fn format_as_written(value: f64, style: &NumberStyle, max_decimals: usize) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    if style.decimal_point {
        return style.render_decimal(value, max_decimals);
    }
    let places = i32::try_from(max_decimals).unwrap_or(9).min(9);
    let scale = 10f64.powi(places);
    let rounded = (value * scale).round() / scale;
    if rounded.fract() == 0.0 {
        style.render_integer(rounded)
    } else {
        NumberStyle {
            decimal_point: true,
            ..style.clone()
        }
        .render_decimal(value, max_decimals)
    }
}

/// The value of `expr` if it is a plain number.
///
/// Macro expressions and variables are not evaluated yet, so anything else
/// gives `None`; the caller reports it.
pub fn literal(expr: &Expr) -> Option<Number> {
    match expr {
        Expr::Number(n) => Some(*n),
        _ => None,
    }
}

/// Reports the block's syntax errors (if any) as [`DiagCode::SyntaxError`]
/// diagnostics and returns `true` if there were any. A block with syntax
/// errors is incomplete and should not be interpreted.
pub fn report_syntax_errors(block: &Block, src: SourceRef, diag: &mut Diagnostics) -> bool {
    for error in &block.errors {
        let col = |c: usize| u16::try_from(c).unwrap_or(u16::MAX);
        let src = SourceRef {
            col_start: col(error.span.start),
            col_end: col(error.span.end),
            ..src
        };
        diag.push(Diagnostic::error(
            DiagCode::SyntaxError,
            error.to_string(),
            src,
        ));
    }
    !block.errors.is_empty()
}
