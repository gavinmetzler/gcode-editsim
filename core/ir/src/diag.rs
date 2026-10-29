//! Diagnostics: warnings and errors tied to a source location.

use crate::segment::SourceRef;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// How serious a diagnostic is. Ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Severity {
    /// Informational note.
    Info,
    /// Something suspicious or controller-dependent; output may still be
    /// usable.
    Warning,
    /// The block could not be interpreted correctly.
    Error,
}

/// Stable, machine-readable diagnostic identifier.
///
/// The numeric values are stored in the render buffer and must never be
/// reused or renumbered. Add new codes at the end with a new number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u16)]
#[non_exhaustive]
pub enum DiagCode {
    /// The text could not be parsed.
    SyntaxError = 1,
    /// A G code the selected dialect does not support.
    UnsupportedGCode = 2,
    /// An M code the selected dialect does not support.
    UnsupportedMCode = 3,
    /// An address word the selected dialect does not support.
    UnsupportedWord = 4,
    /// Two codes from the same modal group in one block.
    ModalGroupConflict = 5,
    /// A word the active command requires is missing.
    MissingWord = 6,
    /// A feed move with no feed rate in effect.
    MissingFeedRate = 7,
    /// Arc start and end are not the same distance from the centre.
    ArcRadiusMismatch = 8,
    /// Arc geometry cannot be constructed from the given words.
    InvalidArc = 9,
    /// A word value is out of range or the wrong type.
    InvalidValue = 10,
    /// Motion outside the machine's travel limits.
    OutOfLimits = 11,
    /// Behaviour differs between controllers; the documented common
    /// behaviour was assumed.
    ControllerDependent = 12,
    /// Recognised but not yet simulated; the program continues without it.
    NotYetSimulated = 13,
}

impl DiagCode {
    /// The stable numeric id.
    pub fn id(self) -> u16 {
        self as u16
    }
}

/// A problem or note produced by any stage of the pipeline.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Diagnostic {
    /// How serious it is.
    pub severity: Severity,
    /// Stable identifier.
    pub code: DiagCode,
    /// Human-readable explanation.
    pub message: String,
    /// Where it happened.
    pub src: SourceRef,
    /// `true` if it depends on the selected dialect's rules rather than on
    /// G-code in general.
    pub dialect_specific: bool,
}

impl Diagnostic {
    /// Creates a diagnostic that is not dialect-specific.
    pub fn new(
        severity: Severity,
        code: DiagCode,
        message: impl Into<String>,
        src: SourceRef,
    ) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            src,
            dialect_specific: false,
        }
    }

    /// Shorthand for an [`Severity::Error`] diagnostic.
    pub fn error(code: DiagCode, message: impl Into<String>, src: SourceRef) -> Self {
        Self::new(Severity::Error, code, message, src)
    }

    /// Shorthand for a [`Severity::Warning`] diagnostic.
    pub fn warning(code: DiagCode, message: impl Into<String>, src: SourceRef) -> Self {
        Self::new(Severity::Warning, code, message, src)
    }

    /// Shorthand for a [`Severity::Info`] diagnostic.
    pub fn info(code: DiagCode, message: impl Into<String>, src: SourceRef) -> Self {
        Self::new(Severity::Info, code, message, src)
    }

    /// Marks the diagnostic as dialect-specific.
    pub fn dialect_specific(mut self) -> Self {
        self.dialect_specific = true;
        self
    }
}

/// Collects diagnostics from any stage, in the order they were reported.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    /// An empty collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    /// Adds an error.
    pub fn error(&mut self, code: DiagCode, message: impl Into<String>, src: SourceRef) {
        self.push(Diagnostic::error(code, message, src));
    }

    /// Adds a warning.
    pub fn warning(&mut self, code: DiagCode, message: impl Into<String>, src: SourceRef) {
        self.push(Diagnostic::warning(code, message, src));
    }

    /// Adds an informational note.
    pub fn info(&mut self, code: DiagCode, message: impl Into<String>, src: SourceRef) {
        self.push(Diagnostic::info(code, message, src));
    }

    /// Number of diagnostics collected.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` if nothing has been collected.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// `true` if any diagnostic is an error.
    pub fn has_errors(&self) -> bool {
        self.count(Severity::Error) > 0
    }

    /// Number of diagnostics with exactly `severity`.
    pub fn count(&self, severity: Severity) -> usize {
        self.items.iter().filter(|d| d.severity == severity).count()
    }

    /// Iterates over the diagnostics in the order they were reported.
    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.items.iter()
    }

    /// The diagnostics as a slice.
    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.items
    }

    /// Consumes the collector, returning the diagnostics.
    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.items
    }
}

impl Extend<Diagnostic> for Diagnostics {
    fn extend<T: IntoIterator<Item = Diagnostic>>(&mut self, iter: T) {
        self.items.extend(iter);
    }
}

impl IntoIterator for Diagnostics {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a Diagnostics {
    type Item = &'a Diagnostic;
    type IntoIter = std::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl From<Diagnostics> for Vec<Diagnostic> {
    fn from(d: Diagnostics) -> Self {
        d.items
    }
}
