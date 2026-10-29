//! Syntax errors.

use thiserror::Error;

use crate::ast::Span;

/// A problem found while parsing one line.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{kind}")]
pub struct SyntaxError {
    /// What went wrong.
    pub kind: SyntaxErrorKind,
    /// Where in the line it went wrong.
    pub span: Span,
}

impl SyntaxError {
    pub(crate) fn new(kind: SyntaxErrorKind, start: usize, end: usize) -> Self {
        Self {
            kind,
            span: Span::new(start, end),
        }
    }
}

/// The kinds of syntax error the parser reports.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SyntaxErrorKind {
    /// A character that cannot start or continue anything at this position.
    #[error("unexpected character {0:?}")]
    UnexpectedCharacter(char),
    /// An address letter with no value after it, e.g. `G` or `XY`.
    #[error("address {0} has no value")]
    MissingValue(char),
    /// A number with no digits, e.g. a lone `.`.
    #[error("malformed number")]
    InvalidNumber,
    /// A number too large to represent.
    #[error("number out of range")]
    NumberOutOfRange,
    /// An `N` word at the start of the block whose value is not a whole
    /// number that fits in 32 bits.
    #[error("sequence number must be a whole number from 0 to 4294967295")]
    InvalidSequenceNumber,
    /// `/0`; block-skip levels run from 1 to 9.
    #[error("block-skip level must be 1 to 9")]
    InvalidBlockSkipLevel,
    /// A `(` with no matching `)` on the same line.
    #[error("comment is not closed with ')'")]
    UnterminatedComment,
    /// A `[` with no matching `]`.
    #[error("missing ']'")]
    UnclosedBracket,
    /// An operand was expected but something else was found.
    #[error("expected a value")]
    ExpectedExpression,
    /// `#` not followed by a variable number or bracketed expression.
    #[error("'#' must be followed by a variable number")]
    MissingVariableNumber,
    /// A variable at the start of a statement not followed by `=`.
    #[error("expected '=' after variable")]
    ExpectedEquals,
    /// A function name not followed by `[`.
    #[error("function {0} must be followed by '['")]
    ExpectedFunctionArguments(String),
    /// Brackets, signs or `#` nested deeper than the parser allows.
    #[error("expression is nested too deeply")]
    NestingTooDeep,
}
