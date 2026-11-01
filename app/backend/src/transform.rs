//! Code transformations as the app asks for them (M4.9).
//!
//! The UI sends the whole program (raw bytes) and a small JSON
//! [`TransformRequest`] naming the selected lines and the transformation,
//! with axes by letter. The machine file maps letters to axes, so neither
//! the UI nor the core names X, Y or Z. The edits and notes come back as a
//! compact binary list ([`encode_edits`]), never JSON, since a selection
//! can be most of a large program (ARCHITECTURE.md §8.4a).
//!
//! # Edit list layout (version 1)
//!
//! Little-endian; every record padded to a multiple of 4 bytes.
//!
//! ```text
//! 0   u32  magic       = 0x45544347 ("GCTE" in memory order)
//! 4   u16  version     = 1
//! 6   u16  reserved    = 0
//! 8   u32  edit_count
//! 12  u32  note_count
//! 16  edits: u32 line, u32 start byte, u32 end byte, u32 text length,
//!            UTF-8 text, padding
//!     notes: u8 severity (0 info, 1 warning, 2 error), 3 bytes padding,
//!            u32 line, u32 message length, UTF-8 message, padding
//! ```
//!
//! Spans are byte offsets within the line (without its line ending), as
//! the parser gives them; the UI converts them to editor positions.

use std::collections::BTreeMap;

use gcode_syntax::Span;
use interpreter::{interpret_traced, Options};
use ir::{ArcPlane, Pos, Severity, MAX_AXES};
use machine_api::{AxisDef, CartesianAxis, Machine};
use serde::Deserialize;
use transform::{transform, Note, TextEdit, TransformError, Transformation, Transformed};

use crate::{load_dialect, load_machine, BackendError, RunOptions};

/// The edit list's magic number: the bytes `GCTE` read as a little-endian
/// `u32`.
pub const EDITS_MAGIC: u32 = u32::from_le_bytes(*b"GCTE");

/// The edit list's layout version.
pub const EDITS_VERSION: u16 = 1;

/// Which lines to transform, and how.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransformRequest {
    /// First selected line (1-based).
    pub first: u32,
    /// Last selected line (1-based, included).
    pub last: u32,
    /// The transformation.
    pub transform: TransformKind,
}

/// A transformation with axes named by letter. Values in mm and degrees,
/// in program coordinates.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum TransformKind {
    /// Move by an offset per axis letter (axes not named: 0).
    Translate {
        /// Offset by axis letter.
        offset: BTreeMap<String, f64>,
    },
    /// Turn by `angle` degrees about `centre` in the plane of two axes
    /// (counter-clockwise from the first toward the second).
    Rotate {
        /// The two axes, in arc-plane order (e.g. `["X", "Y"]`).
        plane: [String; 2],
        /// Degrees, counter-clockwise.
        angle: f64,
        /// The centre by axis letter (axes not named: 0).
        centre: BTreeMap<String, f64>,
    },
    /// Scale the two axes of a plane by `factor` about `centre`.
    Scale {
        /// The two axes (e.g. `["X", "Y"]`).
        plane: [String; 2],
        /// The factor, greater than zero.
        factor: f64,
        /// The centre by axis letter (axes not named: 0).
        centre: BTreeMap<String, f64>,
    },
}

/// The machine axis a letter names (any case), if it is one.
fn axis_index(machine: &dyn Machine, letter: &str) -> Result<usize, BackendError> {
    let mut chars = letter.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return Err(BackendError::InvalidRequest(format!(
            "{letter:?} is not an axis letter"
        )));
    };
    machine
        .axes()
        .iter()
        .position(|a| a.name.eq_ignore_ascii_case(&c))
        .ok_or_else(|| {
            BackendError::InvalidRequest(format!(
                "this machine has no {} axis",
                c.to_ascii_uppercase()
            ))
        })
}

/// Values by axis letter as a position; letters with a zero value need not
/// be axes of the machine.
fn by_axis(machine: &dyn Machine, values: &BTreeMap<String, f64>) -> Result<Pos, BackendError> {
    let mut p: Pos = [0.0; MAX_AXES];
    for (letter, &v) in values {
        if v == 0.0 {
            continue;
        }
        let i = axis_index(machine, letter)?;
        if let Some(slot) = p.get_mut(i) {
            *slot = v;
        }
    }
    Ok(p)
}

fn plane(machine: &dyn Machine, letters: &[String; 2]) -> Result<ArcPlane, BackendError> {
    let index = |l: &String| -> Result<u8, BackendError> {
        u8::try_from(axis_index(machine, l)?)
            .map_err(|_| BackendError::InvalidRequest("axis index out of range".to_owned()))
    };
    Ok(ArcPlane {
        first: index(&letters[0])?,
        second: index(&letters[1])?,
    })
}

fn transformation(
    machine: &dyn Machine,
    kind: &TransformKind,
) -> Result<Transformation, BackendError> {
    Ok(match kind {
        TransformKind::Translate { offset } => Transformation::Translate {
            offset: by_axis(machine, offset)?,
        },
        TransformKind::Rotate {
            plane: p,
            angle,
            centre,
        } => Transformation::Rotate {
            plane: plane(machine, p)?,
            angle: *angle,
            centre: by_axis(machine, centre)?,
        },
        TransformKind::Scale {
            plane: p,
            factor,
            centre,
        } => Transformation::Scale {
            plane: plane(machine, p)?,
            factor: *factor,
            centre: by_axis(machine, centre)?,
        },
    })
}

/// Works out the edits for `request` on `program`, run with the dialect
/// `dialect_id` on the machine `machine_toml`, and returns them as the
/// binary edit list. A transformation that cannot be done is
/// [`BackendError::TransformRefused`], naming the line; nothing is changed.
pub fn transform_program(
    program: &[u8],
    machine_toml: &str,
    dialect_id: &str,
    options: RunOptions,
    request: &TransformRequest,
) -> Result<Vec<u8>, BackendError> {
    let machine = load_machine(machine_toml)?;
    let dialect = load_dialect(dialect_id)?;
    let t = transformation(machine.as_ref(), &request.transform)?;
    let result = transform(
        program,
        dialect.as_ref(),
        machine.as_ref(),
        interpreter_options(options),
        request.first..=request.last,
        &t,
    )
    .map_err(|e| match e {
        TransformError::Refused { .. } => BackendError::TransformRefused(e.to_string()),
    })?;
    Ok(encode_edits(&result))
}

fn interpreter_options(options: RunOptions) -> Options {
    Options {
        block_delete: options.block_delete,
        ..Options::default()
    }
}

/// A point picked in the view (machine coordinates as the view draws them:
/// Cartesian x, y, z, mm) in the program coordinates of line `line`: less
/// the origin of the work offset in effect for that line. One value per
/// machine axis that has a Cartesian direction, by letter. An error if the
/// line does not run.
pub fn program_point(
    program: &[u8],
    machine_toml: &str,
    dialect_id: &str,
    options: RunOptions,
    line: u32,
    point: [f64; 3],
) -> Result<Vec<(char, f64)>, BackendError> {
    let machine = load_machine(machine_toml)?;
    let dialect = load_dialect(dialect_id)?;
    let (_, trace) = interpret_traced(
        program,
        "program",
        dialect.as_ref(),
        machine.as_ref(),
        interpreter_options(options),
        line..=line,
    );
    let entry = trace.first().ok_or_else(|| {
        BackendError::InvalidRequest(format!("line {line} is not run, so it has no coordinates"))
    })?;
    let origin = machine
        .work_offset(entry.state.work_offset)
        .unwrap_or([0.0; MAX_AXES]);
    Ok(machine
        .axes()
        .iter()
        .enumerate()
        .filter_map(|(i, axis): (usize, &AxisDef)| {
            let value = match axis.direction? {
                CartesianAxis::X => point[0],
                CartesianAxis::Y => point[1],
                CartesianAxis::Z => point[2],
            };
            Some((axis.name, value - origin.get(i).copied().unwrap_or(0.0)))
        })
        .collect())
}

fn pad(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The edits and notes as the binary edit list (layout in the module
/// documentation).
pub fn encode_edits(t: &Transformed) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&EDITS_MAGIC.to_le_bytes());
    out.extend_from_slice(&EDITS_VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&u32_of(t.edits.len()).to_le_bytes());
    out.extend_from_slice(&u32_of(t.notes.len()).to_le_bytes());
    for e in &t.edits {
        for n in [
            e.line,
            u32_of(e.span.start),
            u32_of(e.span.end),
            u32_of(e.text.len()),
        ] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(e.text.as_bytes());
        pad(&mut out);
    }
    for n in &t.notes {
        out.push(match n.severity {
            Severity::Info => 0,
            Severity::Warning => 1,
            Severity::Error => 2,
        });
        out.extend_from_slice(&[0, 0, 0]);
        out.extend_from_slice(&n.line.to_le_bytes());
        out.extend_from_slice(&u32_of(n.message.len()).to_le_bytes());
        out.extend_from_slice(n.message.as_bytes());
        pad(&mut out);
    }
    out
}

/// A decoded edit list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EditList {
    /// The edits.
    pub edits: Vec<TextEdit>,
    /// The notes.
    pub notes: Vec<Note>,
}

/// Reads an edit list written by [`encode_edits`]. `None` if it is damaged
/// or of another version. (The app's own tests use it; the UI has its own
/// decoder.)
pub fn decode_edits(bytes: &[u8]) -> Option<EditList> {
    let mut at = 0usize;
    let u32_at = |at: &mut usize| -> Option<u32> {
        let b = bytes.get(*at..*at + 4)?;
        *at += 4;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if u32_at(&mut at)? != EDITS_MAGIC {
        return None;
    }
    let version = u32_at(&mut at)?;
    if version & 0xffff != u32::from(EDITS_VERSION) {
        return None;
    }
    let edit_count = u32_at(&mut at)?;
    let note_count = u32_at(&mut at)?;
    let text = |at: &mut usize, len: u32| -> Option<String> {
        let len = len as usize;
        let s = std::str::from_utf8(bytes.get(*at..*at + len)?)
            .ok()?
            .to_owned();
        *at += len;
        *at = at.div_ceil(4) * 4;
        (*at <= bytes.len()).then_some(s)
    };
    let mut list = EditList::default();
    for _ in 0..edit_count {
        let line = u32_at(&mut at)?;
        let start = u32_at(&mut at)? as usize;
        let end = u32_at(&mut at)? as usize;
        let len = u32_at(&mut at)?;
        list.edits.push(TextEdit {
            line,
            span: Span::new(start, end),
            text: text(&mut at, len)?,
        });
    }
    for _ in 0..note_count {
        let severity = match bytes.get(at)? {
            0 => Severity::Info,
            1 => Severity::Warning,
            2 => Severity::Error,
            _ => return None,
        };
        at += 4;
        let line = u32_at(&mut at)?;
        let len = u32_at(&mut at)?;
        list.notes.push(Note {
            severity,
            line,
            message: text(&mut at, len)?,
        });
    }
    (at == bytes.len()).then_some(list)
}
