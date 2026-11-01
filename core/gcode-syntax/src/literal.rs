//! How a number literal was written, so that a new value can be written
//! the same way (M4.4: rewriting a word changes only its number, and keeps
//! the program's style). Lexical only: what a number *means* (units, a
//! controller's decimal-point rule) is the dialect's business.

use crate::ast::{Expr, Span, Word};

/// Whitespace inside a word, as the parser skips it.
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | 0x0b | 0x0c)
}

/// The written form of a number literal such as `-012.50`, `+.5` or `10`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberStyle {
    /// Written with a `-` sign.
    pub negative: bool,
    /// Written with a `+` sign.
    pub plus: bool,
    /// Whitespace between the sign and the digits (`X- 5.`), usually empty.
    pub sign_gap: String,
    /// Digits before the decimal point, leading zeros included (`012.5`
    /// has 3, `.5` has 0).
    pub int_digits: usize,
    /// Written with leading zeros (`012.5`, `0010`): new values are padded
    /// to the same number of digits before the point.
    pub zero_padded: bool,
    /// Written with a decimal point.
    pub decimal_point: bool,
    /// Digits after the decimal point.
    pub decimals: usize,
}

impl NumberStyle {
    /// The style of `text`, a whole literal: an optional sign (with any
    /// whitespace after it), digits, and an optional decimal point with
    /// more digits; at least one digit. `None` for anything else.
    pub fn of(text: &[u8]) -> Option<Self> {
        let (negative, plus, rest) = match text.split_first() {
            Some((b'-', rest)) => (true, false, rest),
            Some((b'+', rest)) => (false, true, rest),
            _ => (false, false, text),
        };
        let gap_len = if negative || plus {
            rest.iter().take_while(|&&b| is_ws(b)).count()
        } else {
            0
        };
        let (gap, digits) = rest.split_at(gap_len);
        let int_digits = digits.iter().take_while(|b| b.is_ascii_digit()).count();
        let after = digits.get(int_digits..)?;
        let (decimal_point, decimals) = match after.split_first() {
            None => (false, 0),
            Some((b'.', frac)) if frac.iter().all(u8::is_ascii_digit) => (true, frac.len()),
            Some(_) => return None,
        };
        if int_digits + decimals == 0 {
            return None;
        }
        Some(Self {
            negative,
            plus,
            sign_gap: String::from_utf8_lossy(gap).into_owned(),
            int_digits,
            zero_padded: int_digits > 1 && digits.first() == Some(&b'0'),
            decimal_point,
            decimals,
        })
    }

    /// `value` written with a decimal point in this style: at least the
    /// decimals written here, more as the value needs up to
    /// `max_decimals`, rounded at the last one. `None` if it is not finite.
    pub fn render_decimal(&self, value: f64, max_decimals: usize) -> Option<String> {
        if !value.is_finite() {
            return None;
        }
        let places = self.decimals.max(max_decimals);
        let text = format!("{:.places$}", value.abs());
        let (int, frac) = text.split_once('.').unwrap_or((text.as_str(), ""));
        let keep = frac.trim_end_matches('0').len().max(self.decimals);
        let frac = frac.get(..keep).unwrap_or(frac);
        let zero = int.bytes().chain(frac.bytes()).all(|b| b == b'0');
        // `.5` style: no zero before the point (there are always decimals
        // then, since a literal has at least one digit).
        let int = if self.int_digits == 0 && int == "0" && !frac.is_empty() {
            String::new()
        } else {
            self.pad(int)
        };
        Some(format!("{}{int}.{frac}", self.sign(value < 0.0 && !zero)))
    }

    /// `n`, a whole number, written without a decimal point in this style.
    /// `None` if it is not a finite whole number.
    pub fn render_integer(&self, n: f64) -> Option<String> {
        if !n.is_finite() || n.fract() != 0.0 {
            return None;
        }
        let int = format!("{:.0}", n.abs());
        let zero = int.bytes().all(|b| b == b'0');
        Some(format!("{}{}", self.sign(n < 0.0 && !zero), self.pad(&int)))
    }

    fn sign(&self, negative: bool) -> String {
        if negative {
            format!("-{}", self.sign_gap)
        } else if self.plus {
            format!("+{}", self.sign_gap)
        } else {
            String::new()
        }
    }

    /// `int` with leading zeros up to the digits written here, if the
    /// literal was written with leading zeros.
    fn pad(&self, int: &str) -> String {
        if self.zero_padded {
            format!("{int:0>width$}", width = self.int_digits)
        } else {
            int.to_owned()
        }
    }
}

impl Word {
    /// Where this word's number is in `line` (the line it was parsed
    /// from): after the letter and any whitespace, sign included. `None`
    /// if the value is not a plain number, or `line` is not its line.
    pub fn value_span(&self, line: &[u8]) -> Option<Span> {
        if !matches!(self.value, Expr::Number(_)) {
            return None;
        }
        let text = line.get(self.span.start..self.span.end)?;
        let (letter, rest) = text.split_first()?;
        if char::from(letter.to_ascii_uppercase()) != self.letter && self.letter != 'O' {
            return None;
        }
        let ws = rest.iter().take_while(|&&b| is_ws(b)).count();
        let start = self.span.start + 1 + ws;
        let value = line.get(start..self.span.end)?;
        NumberStyle::of(value).map(|_| Span::new(start, self.span.end))
    }
}
