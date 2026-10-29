//! Lexer, parser and expression AST for G-code text. Knows no G-code semantics.
//!
//! [`parse`] turns program text into a [`Program`]: one [`Block`] per
//! non-blank line, each holding its sequence number, block-skip level,
//! address [`Word`]s, variable [`Assignment`]s and [`Comment`]s. Macro
//! expressions such as `#100` or `[#1+2]` become an [`Expr`] tree but are
//! never evaluated here.
//!
//! The parser takes untrusted input. It never panics: problems are recorded
//! as [`SyntaxError`]s on the block they belong to and parsing continues with
//! the next line.
//!
//! ```
//! let program = gcode_syntax::parse_str("N10 G1 X-10.5 Y.5 (cut)\n");
//! let block = &program.blocks[0];
//! assert_eq!(block.line, 1);
//! assert_eq!(block.sequence_number, Some(10));
//! assert_eq!(block.words.len(), 3);
//! assert_eq!(block.comments[0].text, "cut");
//! assert!(block.errors.is_empty());
//! ```

#![forbid(unsafe_code)]

mod ast;
mod error;
mod parser;

pub use ast::{Assignment, BinOp, Block, Comment, CommentKind, Expr, Number, Program, Span, Word};
pub use error::{SyntaxError, SyntaxErrorKind};
pub use parser::{parse, parse_str, MAX_DEPTH};

#[cfg(test)]
mod tests;
