//! The machine's axes, as far as a dialect needs to know them.

use ir::{ArcPlane, MAX_AXES};
use thiserror::Error;

/// One machine axis, seen from the dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisInfo {
    /// The address letter that programs this axis, e.g. `'X'` or `'A'`.
    pub letter: char,
    /// The address letter that gives an arc-centre offset along this axis,
    /// if the axis can take part in arcs (e.g. `'I'` for `'X'`).
    pub arc_offset_letter: Option<char>,
    /// `true` for a rotary axis (values in degrees, never unit-converted);
    /// `false` for a linear axis (values in length units).
    pub rotary: bool,
}

/// The ordered list of axes a machine provides.
///
/// Index `i` in this list is index `i` in [`ir::Pos`]. Dialects look axes up
/// here instead of assuming any particular letters exist. It is built from
/// the machine definition by whoever drives the dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisSet {
    axes: Vec<AxisInfo>,
}

/// Why an [`AxisSet`] could not be built.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AxisSetError {
    /// More axes than [`MAX_AXES`].
    #[error("{0} axes given, at most {MAX_AXES} are supported")]
    TooMany(usize),
    /// A letter that is not an upper-case ASCII letter.
    #[error("{0:?} is not an upper-case letter A-Z")]
    InvalidLetter(char),
    /// The same letter used twice (as an axis or arc-offset letter).
    #[error("letter {0} is used more than once")]
    Duplicate(char),
}

impl AxisSet {
    /// Builds an axis set, checking that there are at most [`MAX_AXES`]
    /// axes and that every axis and arc-offset letter is a distinct
    /// upper-case ASCII letter.
    pub fn new(axes: Vec<AxisInfo>) -> Result<Self, AxisSetError> {
        if axes.len() > MAX_AXES {
            return Err(AxisSetError::TooMany(axes.len()));
        }
        let mut seen = Vec::new();
        for axis in &axes {
            for letter in std::iter::once(axis.letter).chain(axis.arc_offset_letter) {
                if !letter.is_ascii_uppercase() {
                    return Err(AxisSetError::InvalidLetter(letter));
                }
                if seen.contains(&letter) {
                    return Err(AxisSetError::Duplicate(letter));
                }
                seen.push(letter);
            }
        }
        Ok(Self { axes })
    }

    /// The axes in machine order.
    pub fn axes(&self) -> &[AxisInfo] {
        &self.axes
    }

    /// Number of axes.
    pub fn len(&self) -> usize {
        self.axes.len()
    }

    /// `true` if the machine has no axes.
    pub fn is_empty(&self) -> bool {
        self.axes.is_empty()
    }

    /// Index of the axis programmed with `letter`.
    pub fn index_of(&self, letter: char) -> Option<usize> {
        self.axes.iter().position(|a| a.letter == letter)
    }

    /// Index of the axis whose arc-centre offset is given by `letter`.
    pub fn index_of_arc_offset(&self, letter: char) -> Option<usize> {
        self.axes
            .iter()
            .position(|a| a.arc_offset_letter == Some(letter))
    }

    /// The arc plane spanned by the axes `first` and `second`, if the
    /// machine has both.
    pub fn plane(&self, first: char, second: char) -> Option<ArcPlane> {
        let first = u8::try_from(self.index_of(first)?).ok()?;
        let second = u8::try_from(self.index_of(second)?).ok()?;
        (first != second).then_some(ArcPlane { first, second })
    }
}
