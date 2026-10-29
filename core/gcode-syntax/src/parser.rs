//! Line-oriented recursive-descent parser.
//!
//! Every function here works on byte slices with `get`/`position` rather
//! than indexing, so no input can make it panic. Recursion only happens
//! inside expressions and is capped at [`MAX_DEPTH`].

use crate::ast::{
    Assignment, BinOp, Block, Comment, CommentKind, Expr, Number, Program, Span, Word,
};
use crate::error::{SyntaxError, SyntaxErrorKind};

/// Maximum nesting of brackets, unary signs, `#` and function calls inside
/// one expression. Deeper input is reported as
/// [`SyntaxErrorKind::NestingTooDeep`] instead of risking a stack overflow.
pub const MAX_DEPTH: usize = 64;

type PResult<T> = Result<T, SyntaxError>;

const COMPARISON_OPS: &[BinOp] = &[
    BinOp::Eq,
    BinOp::Ne,
    BinOp::Gt,
    BinOp::Ge,
    BinOp::Lt,
    BinOp::Le,
];
const ADDITIVE_OPS: &[BinOp] = &[BinOp::Add, BinOp::Sub, BinOp::Or, BinOp::Xor];
const MULTIPLICATIVE_OPS: &[BinOp] = &[BinOp::Mul, BinOp::Div, BinOp::And, BinOp::Mod];

/// Parses a whole program.
///
/// Lines are separated by `\n`; a `\r` directly before it is dropped. Blank
/// lines produce no block. Problems are recorded on the block they occur in
/// and parsing always continues with the next line, so this never fails.
pub fn parse(src: &[u8]) -> Program {
    let blocks = src
        .split(|&b| b == b'\n')
        .enumerate()
        .filter_map(|(index, raw)| {
            let line = raw.strip_suffix(b"\r").unwrap_or(raw);
            let line_no = u32::try_from(index)
                .ok()
                .and_then(|n| n.checked_add(1))
                .unwrap_or(u32::MAX);
            parse_line(line, line_no)
        })
        .collect();
    Program { blocks }
}

/// Convenience wrapper around [`parse`] for text that is already a `str`.
pub fn parse_str(src: &str) -> Program {
    parse(src.as_bytes())
}

fn parse_line(line: &[u8], line_no: u32) -> Option<Block> {
    let mut c = Cursor::new(line);
    c.skip_ws();
    c.peek()?;
    let mut block = Block {
        line: line_no,
        ..Block::default()
    };
    if let Err(e) = parse_items(&mut c, &mut block) {
        block.errors.push(e);
    }
    Some(block)
}

fn parse_items(c: &mut Cursor<'_>, block: &mut Block) -> PResult<()> {
    let skip_start = c.pos;
    if c.eat(b'/') {
        let level = match c.peek() {
            Some(d @ b'1'..=b'9') => {
                c.bump();
                d - b'0'
            }
            Some(b'0') => {
                c.bump();
                return Err(SyntaxError::new(
                    SyntaxErrorKind::InvalidBlockSkipLevel,
                    skip_start,
                    c.pos,
                ));
            }
            _ => 1,
        };
        block.block_skip = Some(level);
    }

    loop {
        c.skip_ws();
        let start = c.pos;
        let Some(b) = c.peek() else {
            return Ok(());
        };
        match b {
            b'(' => {
                c.bump();
                let rest = c.rest();
                match rest.iter().position(|&b| b == b')') {
                    Some(close) => {
                        let text = rest.get(..close).unwrap_or_default();
                        block.comments.push(comment(
                            CommentKind::Paren,
                            text,
                            start,
                            c.pos + close + 1,
                        ));
                        c.pos += close + 1;
                    }
                    None => {
                        block
                            .comments
                            .push(comment(CommentKind::Paren, rest, start, c.src.len()));
                        c.pos = c.src.len();
                        return Err(SyntaxError::new(
                            SyntaxErrorKind::UnterminatedComment,
                            start,
                            start + 1,
                        ));
                    }
                }
            }
            b';' => {
                c.bump();
                block.comments.push(comment(
                    CommentKind::Semicolon,
                    c.rest(),
                    start,
                    c.src.len(),
                ));
                c.pos = c.src.len();
            }
            b'%' => {
                c.bump();
                block.program_delimiter = true;
            }
            b'#' => {
                let assignment = parse_assignment(c)?;
                block.assignments.push(assignment);
            }
            b if b.is_ascii_alphabetic() => {
                let word = parse_word(c)?;
                let is_first = block.sequence_number.is_none()
                    && block.words.is_empty()
                    && block.assignments.is_empty();
                if word.letter == 'N' && is_first {
                    block.sequence_number = Some(sequence_number(&word)?);
                } else {
                    block.words.push(word);
                }
            }
            _ => return Err(c.unexpected()),
        }
    }
}

fn comment(kind: CommentKind, text: &[u8], start: usize, end: usize) -> Comment {
    Comment {
        kind,
        text: String::from_utf8_lossy(text).into_owned(),
        span: Span::new(start, end),
    }
}

fn sequence_number(word: &Word) -> PResult<u32> {
    let err = || SyntaxError {
        kind: SyntaxErrorKind::InvalidSequenceNumber,
        span: word.span,
    };
    match &word.value {
        Expr::Number(Number {
            value,
            decimal_point: false,
        }) if *value >= 0.0 && *value <= f64::from(u32::MAX) => {
            // The value has no fractional part (no decimal point) and is in
            // range, so the conversion is exact.
            let n = *value as u32;
            Ok(n)
        }
        _ => Err(err()),
    }
}

fn parse_word(c: &mut Cursor<'_>) -> PResult<Word> {
    let start = c.pos;
    let letter = c.peek().map_or('?', |b| char::from(b.to_ascii_uppercase()));
    c.bump();
    c.skip_ws();

    let sign = match c.peek() {
        Some(b'-') => Some(true),
        Some(b'+') => Some(false),
        _ => None,
    };
    if sign.is_some() {
        c.bump();
        c.skip_ws();
    }
    let operand = match c.peek() {
        Some(b'0'..=b'9' | b'.') => Expr::Number(parse_number(c)?),
        Some(b'#') => parse_var(c, 0)?,
        Some(b'[') => parse_bracketed(c, 0)?,
        _ if sign.is_some() => return Err(c.error_here(SyntaxErrorKind::ExpectedExpression)),
        _ => {
            return Err(SyntaxError::new(
                SyntaxErrorKind::MissingValue(letter),
                start,
                start + 1,
            ))
        }
    };
    let value = if sign == Some(true) {
        negate(operand)
    } else {
        operand
    };
    Ok(Word {
        letter,
        value,
        span: Span::new(start, c.pos),
    })
}

fn parse_assignment(c: &mut Cursor<'_>) -> PResult<Assignment> {
    let start = c.pos;
    let target = parse_var(c, 0)?;
    c.skip_ws();
    if !c.eat(b'=') {
        return Err(c.error_here(SyntaxErrorKind::ExpectedEquals));
    }
    let value = parse_expr(c, 0)?;
    Ok(Assignment {
        target,
        value,
        span: Span::new(start, c.pos),
    })
}

fn negate(e: Expr) -> Expr {
    match e {
        Expr::Number(n) => Expr::Number(Number {
            value: -n.value,
            ..n
        }),
        other => Expr::Neg(Box::new(other)),
    }
}

fn parse_number(c: &mut Cursor<'_>) -> PResult<Number> {
    let start = c.pos;
    c.eat_while(|b| b.is_ascii_digit());
    let decimal_point = c.eat(b'.');
    if decimal_point {
        c.eat_while(|b| b.is_ascii_digit());
    }
    let text = c.src.get(start..c.pos).unwrap_or_default();
    let invalid = || SyntaxError::new(SyntaxErrorKind::InvalidNumber, start, c.pos);
    if !text.iter().any(u8::is_ascii_digit) {
        return Err(invalid());
    }
    let value: f64 = std::str::from_utf8(text)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(invalid)?;
    if !value.is_finite() {
        return Err(SyntaxError::new(
            SyntaxErrorKind::NumberOutOfRange,
            start,
            c.pos,
        ));
    }
    Ok(Number {
        value,
        decimal_point,
    })
}

fn check_depth(c: &Cursor<'_>, depth: usize) -> PResult<()> {
    if depth > MAX_DEPTH {
        Err(c.error_here(SyntaxErrorKind::NestingTooDeep))
    } else {
        Ok(())
    }
}

/// Lowest precedence: comparisons.
fn parse_expr(c: &mut Cursor<'_>, depth: usize) -> PResult<Expr> {
    let mut lhs = parse_additive(c, depth)?;
    while let Some(op) = c.try_op(COMPARISON_OPS) {
        let rhs = parse_additive(c, depth)?;
        lhs = binary(op, lhs, rhs);
    }
    Ok(lhs)
}

fn parse_additive(c: &mut Cursor<'_>, depth: usize) -> PResult<Expr> {
    let mut lhs = parse_term(c, depth)?;
    while let Some(op) = c.try_op(ADDITIVE_OPS) {
        let rhs = parse_term(c, depth)?;
        lhs = binary(op, lhs, rhs);
    }
    Ok(lhs)
}

fn parse_term(c: &mut Cursor<'_>, depth: usize) -> PResult<Expr> {
    let mut lhs = parse_unary(c, depth)?;
    while let Some(op) = c.try_op(MULTIPLICATIVE_OPS) {
        let rhs = parse_unary(c, depth)?;
        lhs = binary(op, lhs, rhs);
    }
    Ok(lhs)
}

fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn parse_unary(c: &mut Cursor<'_>, depth: usize) -> PResult<Expr> {
    check_depth(c, depth)?;
    c.skip_ws();
    if c.eat(b'-') {
        Ok(negate(parse_unary(c, depth + 1)?))
    } else if c.eat(b'+') {
        parse_unary(c, depth + 1)
    } else {
        parse_primary(c, depth)
    }
}

fn parse_primary(c: &mut Cursor<'_>, depth: usize) -> PResult<Expr> {
    c.skip_ws();
    match c.peek() {
        Some(b'0'..=b'9' | b'.') => Ok(Expr::Number(parse_number(c)?)),
        Some(b'#') => parse_var(c, depth + 1),
        Some(b'[') => parse_bracketed(c, depth + 1),
        Some(b) if b.is_ascii_alphabetic() => parse_call(c, depth + 1),
        _ => Err(c.error_here(SyntaxErrorKind::ExpectedExpression)),
    }
}

fn parse_var(c: &mut Cursor<'_>, depth: usize) -> PResult<Expr> {
    check_depth(c, depth)?;
    c.bump(); // '#'
    c.skip_ws();
    let index = match c.peek() {
        Some(b'0'..=b'9' | b'.') => Expr::Number(parse_number(c)?),
        Some(b'[') => parse_bracketed(c, depth + 1)?,
        Some(b'#') => parse_var(c, depth + 1)?,
        _ => return Err(c.error_here(SyntaxErrorKind::MissingVariableNumber)),
    };
    Ok(Expr::Var(Box::new(index)))
}

fn parse_bracketed(c: &mut Cursor<'_>, depth: usize) -> PResult<Expr> {
    check_depth(c, depth)?;
    let open = c.pos;
    c.bump(); // '['
    let inner = parse_expr(c, depth + 1)?;
    c.skip_ws();
    if c.eat(b']') {
        Ok(inner)
    } else {
        Err(SyntaxError::new(
            SyntaxErrorKind::UnclosedBracket,
            open,
            open + 1,
        ))
    }
}

fn parse_call(c: &mut Cursor<'_>, depth: usize) -> PResult<Expr> {
    check_depth(c, depth)?;
    let start = c.pos;
    c.eat_while(|b| b.is_ascii_alphabetic());
    let name =
        String::from_utf8_lossy(c.src.get(start..c.pos).unwrap_or_default()).to_ascii_uppercase();
    c.skip_ws();
    let open = c.pos;
    if !c.eat(b'[') {
        return Err(SyntaxError::new(
            SyntaxErrorKind::ExpectedFunctionArguments(name),
            start,
            c.pos,
        ));
    }
    let mut args = Vec::new();
    loop {
        args.push(parse_expr(c, depth + 1)?);
        c.skip_ws();
        if c.eat(b',') {
            continue;
        }
        if c.eat(b']') {
            break;
        }
        return Err(SyntaxError::new(
            SyntaxErrorKind::UnclosedBracket,
            open,
            open + 1,
        ));
    }
    Ok(Expr::Call { name, args })
}

/// A position within one line.
struct Cursor<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(src: &'a [u8]) -> Self {
        Self { src, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn rest(&self) -> &'a [u8] {
        self.src.get(self.pos..).unwrap_or_default()
    }

    fn bump(&mut self) {
        if self.pos < self.src.len() {
            self.pos += 1;
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_while(&mut self, pred: impl Fn(u8) -> bool) {
        while self.peek().is_some_and(&pred) {
            self.bump();
        }
    }

    fn skip_ws(&mut self) {
        self.eat_while(|b| matches!(b, b' ' | b'\t' | b'\r' | 0x0b | 0x0c));
    }

    /// Consumes one of `ops` if it comes next (after optional whitespace).
    /// Symbol operators are one character; keyword operators (`AND`, `EQ`,
    /// ...) must match a whole run of letters, case-insensitively. Leaves
    /// the cursor untouched if nothing matches.
    fn try_op(&mut self, ops: &[BinOp]) -> Option<BinOp> {
        let saved = self.pos;
        self.skip_ws();
        let rest = self.rest();
        let token_len = match rest.first() {
            Some(b) if b.is_ascii_alphabetic() => rest
                .iter()
                .position(|b| !b.is_ascii_alphabetic())
                .unwrap_or(rest.len()),
            Some(_) => 1,
            None => 0,
        };
        let token = rest.get(..token_len).unwrap_or_default();
        match ops
            .iter()
            .find(|op| token.eq_ignore_ascii_case(op.symbol().as_bytes()))
        {
            Some(&op) if token_len > 0 => {
                self.pos += token_len;
                Some(op)
            }
            _ => {
                self.pos = saved;
                None
            }
        }
    }

    /// An error spanning the character at the cursor (empty at end of line).
    fn error_here(&self, kind: SyntaxErrorKind) -> SyntaxError {
        let (_, len) = self.current_char();
        SyntaxError::new(kind, self.pos, self.pos + len)
    }

    fn unexpected(&self) -> SyntaxError {
        let (ch, len) = self.current_char();
        SyntaxError::new(
            SyntaxErrorKind::UnexpectedCharacter(ch),
            self.pos,
            self.pos + len,
        )
    }

    /// Decodes the character at the cursor, returning it and its length in
    /// bytes. Invalid UTF-8 decodes as U+FFFD with length 1; end of line
    /// gives length 0.
    fn current_char(&self) -> (char, usize) {
        let rest = self.rest();
        let window = rest.get(..rest.len().min(4)).unwrap_or_default();
        match window
            .utf8_chunks()
            .next()
            .and_then(|chunk| chunk.valid().chars().next())
        {
            Some(ch) => (ch, ch.len_utf8()),
            None if rest.is_empty() => (char::REPLACEMENT_CHARACTER, 0),
            None => (char::REPLACEMENT_CHARACTER, 1),
        }
    }
}
