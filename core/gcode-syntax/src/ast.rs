//! Syntax tree produced by the parser.
//!
//! These types describe the *shape* of the text only. Nothing here knows what
//! `G1` or `M3` means; that is the dialect's job.

use std::fmt;

use crate::error::SyntaxError;

/// A half-open byte range `start..end` within a single source line.
///
/// Offsets are 0-based byte offsets from the start of the line (not
/// character counts), so they stay exact even if the line contains
/// non-ASCII bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// First byte of the range.
    pub start: usize,
    /// One past the last byte of the range.
    pub end: usize,
}

impl Span {
    /// Creates a span covering `start..end`.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// The result of parsing a whole program.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    /// One entry per non-blank source line, in source order.
    pub blocks: Vec<Block>,
}

impl Program {
    /// Iterates over every syntax error in the program together with the
    /// 1-based line it belongs to.
    pub fn errors(&self) -> impl Iterator<Item = (u32, &SyntaxError)> {
        self.blocks
            .iter()
            .flat_map(|b| b.errors.iter().map(move |e| (b.line, e)))
    }
}

/// One line of G-code.
///
/// If `errors` is non-empty the block is incomplete: parsing of the line
/// stopped at the first error, so `words` and `assignments` hold only what
/// came before it. Consumers should not execute such a block.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    /// 1-based source line number.
    pub line: u32,
    /// Block-skip level if the line starts with `/` (level 1) or `/n`
    /// (level `n`, 1–9).
    pub block_skip: Option<u8>,
    /// Sequence number from an `N` word at the start of the block.
    pub sequence_number: Option<u32>,
    /// Address words in source order, excluding the leading `N` word.
    pub words: Vec<Word>,
    /// Variable assignments such as `#100=5`, in source order.
    pub assignments: Vec<Assignment>,
    /// Comments in source order.
    pub comments: Vec<Comment>,
    /// `true` if the line contained a `%` program delimiter.
    pub program_delimiter: bool,
    /// Problems found on this line. Parsing of the line stops at the first
    /// error that leaves the rest of the line ambiguous.
    pub errors: Vec<SyntaxError>,
}

/// An address letter with its value, for example `G1`, `X-10.5` or `Z#100`.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    /// Address letter, always upper-case ASCII.
    pub letter: char,
    /// The value as written (a plain number in the common case).
    pub value: Expr,
    /// Position of the whole word in the line.
    pub span: Span,
}

/// A variable assignment such as `#100 = #1 + 2`.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// The variable being assigned. Always an [`Expr::Var`].
    pub target: Expr,
    /// The (unevaluated) value.
    pub value: Expr,
    /// Position of the whole assignment in the line.
    pub span: Span,
}

/// Which syntax a comment used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentKind {
    /// `( ... )`
    Paren,
    /// `; ...` to the end of the line.
    Semicolon,
}

/// A comment, with its delimiters removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// Comment syntax.
    pub kind: CommentKind,
    /// Text between the delimiters, unmodified except that invalid UTF-8 is
    /// replaced with U+FFFD.
    pub text: String,
    /// Position of the comment in the line, including its delimiters.
    pub span: Span,
}

/// A numeric literal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Number {
    /// Parsed value. Always finite.
    pub value: f64,
    /// Whether the literal was written with a decimal point (`10.` versus
    /// `10`). Some controllers scale numbers without a decimal point, so this
    /// is kept for the dialect to decide.
    pub decimal_point: bool,
}

/// Binary operators allowed inside expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `MOD`
    Mod,
    /// `AND`
    And,
    /// `OR`
    Or,
    /// `XOR`
    Xor,
    /// `EQ`
    Eq,
    /// `NE`
    Ne,
    /// `GT`
    Gt,
    /// `GE`
    Ge,
    /// `LT`
    Lt,
    /// `LE`
    Le,
}

impl BinOp {
    /// The operator as it is written in G-code.
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "MOD",
            BinOp::And => "AND",
            BinOp::Or => "OR",
            BinOp::Xor => "XOR",
            BinOp::Eq => "EQ",
            BinOp::Ne => "NE",
            BinOp::Gt => "GT",
            BinOp::Ge => "GE",
            BinOp::Lt => "LT",
            BinOp::Le => "LE",
        }
    }
}

/// An unevaluated expression.
///
/// Brackets used only for grouping do not appear in the tree; the tree shape
/// already records the grouping.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A numeric literal. A sign directly in front of a literal is folded
    /// into it, so `-5` is `Number(-5)` rather than `Neg(Number(5))`.
    Number(Number),
    /// A variable reference `#n`; the boxed expression is the variable
    /// number (`#100`, `#[#1+1]`).
    Var(Box<Expr>),
    /// Unary minus applied to a non-literal, e.g. `-#1`.
    Neg(Box<Expr>),
    /// A binary operation.
    Binary {
        /// The operator.
        op: BinOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
    /// A function call such as `SIN[30]`. The name is upper-cased but not
    /// checked: the parser accepts any name.
    Call {
        /// Upper-case function name.
        name: String,
        /// Comma-separated arguments inside the brackets.
        args: Vec<Expr>,
    },
}

impl Expr {
    /// Convenience constructor for a numeric literal.
    pub fn number(value: f64, decimal_point: bool) -> Self {
        Expr::Number(Number {
            value,
            decimal_point,
        })
    }
}

impl fmt::Display for Number {
    /// Writes the number so that parsing it again gives the same value and
    /// `decimal_point` flag (`10`, `10.`, `10.5`, `-0.25`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self.value.to_string();
        if self.decimal_point && !text.contains('.') {
            write!(f, "{text}.")
        } else {
            f.write_str(&text)
        }
    }
}

impl fmt::Display for Expr {
    /// Writes the expression in G-code syntax with every binary operation
    /// bracketed, e.g. `[#1 + [2 * 3]]`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Number(n) => n.fmt(f),
            Expr::Var(inner) => match inner.as_ref() {
                Expr::Number(n) if n.value.is_sign_positive() => write!(f, "#{n}"),
                Expr::Binary { .. } => write!(f, "#{inner}"),
                _ => write!(f, "#[{inner}]"),
            },
            Expr::Neg(inner) => write!(f, "-{inner}"),
            Expr::Binary { op, lhs, rhs } => write!(f, "[{lhs} {} {rhs}]", op.symbol()),
            Expr::Call { name, args } => {
                write!(f, "{name}[")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    arg.fmt(f)?;
                }
                f.write_str("]")
            }
        }
    }
}
