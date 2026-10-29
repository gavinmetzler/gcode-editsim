//! Small building blocks shared by dialect implementations.

use gcode_syntax::{Block, Expr, Number};
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
