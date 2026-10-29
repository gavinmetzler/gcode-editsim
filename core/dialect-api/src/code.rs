//! Identifiers for G and M codes.

use std::fmt;

/// A code such as `G1`, `G54.1` or `M30`, normalised so that `G1`, `G01`
/// and `G1.0` are the same key.
///
/// The number is stored in tenths, which covers the one decimal place
/// that codes use (`G54.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CodeKey {
    letter: char,
    tenths: u32,
}

impl CodeKey {
    /// Builds a key from an address letter and a numeric value, as found in
    /// a parsed word. Returns `None` if the letter is not ASCII alphabetic or
    /// the value is negative, not finite, too large, or has more than one
    /// significant decimal place.
    pub fn from_value(letter: char, value: f64) -> Option<Self> {
        if !letter.is_ascii_alphabetic() || !value.is_finite() || value < 0.0 {
            return None;
        }
        let scaled = value * 10.0;
        let rounded = scaled.round();
        if (scaled - rounded).abs() > 1e-6 || rounded > f64::from(u32::MAX) {
            return None;
        }
        Some(Self {
            letter: letter.to_ascii_uppercase(),
            // In range and integral (checked above), so the cast is exact.
            tenths: rounded as u32,
        })
    }

    /// Parses text such as `"G01"`, `"g54.1"` or `"M30"`.
    pub fn parse(text: &str) -> Option<Self> {
        let mut chars = text.chars();
        let letter = chars.next()?;
        let number = chars.as_str();
        let valid = !number.is_empty()
            && number.chars().all(|c| c.is_ascii_digit() || c == '.')
            && number.chars().filter(|&c| c == '.').count() <= 1
            && number.chars().any(|c| c.is_ascii_digit());
        if !valid {
            return None;
        }
        Self::from_value(letter, number.parse().ok()?)
    }

    /// The address letter, upper-case.
    pub fn letter(self) -> char {
        self.letter
    }

    /// The numeric part, e.g. `54.1` for `G54.1`.
    pub fn value(self) -> f64 {
        f64::from(self.tenths) / 10.0
    }
}

impl fmt::Display for CodeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (whole, tenth) = (self.tenths / 10, self.tenths % 10);
        if tenth == 0 {
            write!(f, "{}{whole}", self.letter)
        } else {
            write!(f, "{}{whole}.{tenth}", self.letter)
        }
    }
}
