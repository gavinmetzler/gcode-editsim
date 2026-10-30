//! Machine definitions: the data a user writes to describe a machine.

use std::collections::BTreeMap;

use ir::{Pos, WorkOffsetId, MAX_AXES};
use serde::Deserialize;
use thiserror::Error;

/// Whether an axis moves in a straight line or rotates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AxisKind {
    /// Straight-line travel, in mm.
    Linear,
    /// Rotation, in degrees.
    Rotary,
}

/// A direction in the machine's Cartesian space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CartesianAxis {
    /// The Cartesian x direction.
    X,
    /// The Cartesian y direction.
    Y,
    /// The Cartesian z direction (usually along the spindle).
    Z,
}

impl CartesianAxis {
    /// Index into an `[x, y, z]` array.
    pub fn index(self) -> usize {
        match self {
            CartesianAxis::X => 0,
            CartesianAxis::Y => 1,
            CartesianAxis::Z => 2,
        }
    }
}

/// One machine axis.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisDef {
    /// Address letter that programs this axis, e.g. `'X'` or `'A'`.
    pub name: char,
    /// Linear or rotary.
    pub kind: AxisKind,
    /// Soft travel limits `(min, max)` in machine coordinates, if known.
    pub travel: Option<(f64, f64)>,
    /// Address letter for arc-centre offsets along this axis (e.g. `'I'`
    /// for `'X'`), if the axis takes part in arcs.
    pub arc_offset_letter: Option<char>,
    /// For a linear axis, the Cartesian direction it moves the tool; for a
    /// rotary axis, the Cartesian axis it rotates about. `None` lets the
    /// machine type choose.
    pub direction: Option<CartesianAxis>,
    /// Rapid traverse rate in mm/min (deg/min for rotary axes), if known.
    pub rapid_rate: Option<f64>,
    /// The axis's reference (home) position in machine coordinates, if
    /// known. Reference-return commands move here, and the simulation
    /// starts here.
    pub reference: Option<f64>,
    /// `true` if values for this axis may be programmed as a diameter.
    pub diameter_mode_capable: bool,
}

/// Machine-wide speed limits.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SpeedLimits {
    /// Maximum cutting feed rate in mm/min, if known.
    pub max_feed: Option<f64>,
    /// Maximum spindle speed in rpm, if known.
    pub max_spindle_rpm: Option<f64>,
}

/// A machine as described by the user, before a machine type checks it.
///
/// Loaded from a TOML file; see `docs/machine-definition.md` for the
/// format.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineDef {
    /// Display name.
    pub name: String,
    /// Machine type that should interpret this definition, e.g. `"mill"`.
    pub family: String,
    /// Axes in the order used by [`ir::Pos`].
    pub axes: Vec<AxisDef>,
    /// Machine-wide speed limits.
    pub speeds: SpeedLimits,
    work_offsets: BTreeMap<WorkOffsetId, Pos>,
}

/// Why a machine definition could not be loaded.
#[derive(Debug, Error)]
pub enum MachineDefError {
    /// Not valid TOML, a missing or misspelled setting, or a wrong type.
    #[error("invalid machine definition: {0}")]
    Toml(#[from] toml::de::Error),
    /// More axes than [`MAX_AXES`].
    #[error("{0} axes defined, at most {MAX_AXES} are supported")]
    TooManyAxes(usize),
    /// A letter setting that is not a single letter A-Z.
    #[error("{0:?} is not a single letter A-Z")]
    InvalidLetter(String),
    /// The same letter used twice, as an axis or arc-offset letter.
    #[error("letter {0} is used more than once")]
    DuplicateLetter(char),
    /// Travel limits that are not finite or have min >= max.
    #[error("axis {axis}: travel must be two finite numbers with min < max")]
    InvalidTravel {
        /// The axis.
        axis: char,
    },
    /// A speed that is not a finite number greater than zero.
    #[error("{what} must be a number greater than zero")]
    InvalidSpeed {
        /// Which setting.
        what: String,
    },
    /// A reference position that is not finite or lies outside the travel.
    #[error("axis {0}: reference must be a finite number within the travel limits")]
    InvalidReference(char),
    /// An arc-offset letter on a rotary axis.
    #[error("axis {0}: rotary axes cannot have an arc offset letter")]
    ArcOffsetOnRotary(char),
    /// A work offset whose key is not a number from 1 to 65535.
    #[error("work offset {0:?}: the key must be a whole number from 1 to 65535")]
    InvalidWorkOffsetId(String),
    /// A work offset naming an axis the machine does not have.
    #[error("work offset {id}: the machine has no axis {axis:?}")]
    UnknownWorkOffsetAxis {
        /// The work offset.
        id: u16,
        /// The letter given.
        axis: String,
    },
    /// A work offset value that is not finite.
    #[error("work offset {id}: value for {axis} must be a finite number")]
    InvalidWorkOffsetValue {
        /// The work offset.
        id: u16,
        /// The axis.
        axis: char,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDef {
    machine: RawMachine,
    #[serde(default)]
    axis: Vec<RawAxis>,
    #[serde(default)]
    work_offsets: BTreeMap<String, BTreeMap<String, f64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMachine {
    name: String,
    family: String,
    #[serde(default)]
    max_feed: Option<f64>,
    #[serde(default)]
    max_spindle_rpm: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAxis {
    letter: String,
    kind: AxisKind,
    #[serde(default)]
    travel: Option<[f64; 2]>,
    #[serde(default)]
    arc_offset: Option<String>,
    #[serde(default)]
    direction: Option<CartesianAxis>,
    #[serde(default)]
    rapid: Option<f64>,
    #[serde(default)]
    reference: Option<f64>,
    #[serde(default)]
    diameter_mode: bool,
}

fn letter(text: &str) -> Result<char, MachineDefError> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Ok(c.to_ascii_uppercase()),
        _ => Err(MachineDefError::InvalidLetter(text.to_owned())),
    }
}

fn speed(
    value: Option<f64>,
    what: impl FnOnce() -> String,
) -> Result<Option<f64>, MachineDefError> {
    match value {
        Some(v) if !(v.is_finite() && v > 0.0) => {
            Err(MachineDefError::InvalidSpeed { what: what() })
        }
        other => Ok(other),
    }
}

impl MachineDef {
    /// Parses and checks a machine definition.
    pub fn from_toml_str(text: &str) -> Result<Self, MachineDefError> {
        let raw: RawDef = toml::from_str(text)?;
        if raw.axis.len() > MAX_AXES {
            return Err(MachineDefError::TooManyAxes(raw.axis.len()));
        }

        let mut used = Vec::new();
        let mut axes = Vec::with_capacity(raw.axis.len());
        for a in raw.axis {
            let name = letter(&a.letter)?;
            let arc_offset_letter = a.arc_offset.as_deref().map(letter).transpose()?;
            for l in std::iter::once(name).chain(arc_offset_letter) {
                if used.contains(&l) {
                    return Err(MachineDefError::DuplicateLetter(l));
                }
                used.push(l);
            }
            if a.kind == AxisKind::Rotary && arc_offset_letter.is_some() {
                return Err(MachineDefError::ArcOffsetOnRotary(name));
            }
            let travel = match a.travel {
                Some([min, max]) if min.is_finite() && max.is_finite() && min < max => {
                    Some((min, max))
                }
                Some(_) => return Err(MachineDefError::InvalidTravel { axis: name }),
                None => None,
            };
            if let Some(r) = a.reference {
                let inside = travel.is_none_or(|(min, max)| r >= min && r <= max);
                if !(r.is_finite() && inside) {
                    return Err(MachineDefError::InvalidReference(name));
                }
            }
            axes.push(AxisDef {
                name,
                kind: a.kind,
                travel,
                arc_offset_letter,
                direction: a.direction,
                rapid_rate: speed(a.rapid, || format!("axis {name}: rapid"))?,
                reference: a.reference,
                diameter_mode_capable: a.diameter_mode,
            });
        }

        let speeds = SpeedLimits {
            max_feed: speed(raw.machine.max_feed, || "max_feed".into())?,
            max_spindle_rpm: speed(raw.machine.max_spindle_rpm, || "max_spindle_rpm".into())?,
        };

        let mut work_offsets = BTreeMap::new();
        for (key, values) in raw.work_offsets {
            let id = key
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|&n| n >= 1)
                .ok_or_else(|| MachineDefError::InvalidWorkOffsetId(key.clone()))?;
            let mut pos = [0.0; MAX_AXES];
            for (axis_text, value) in values {
                let index = letter(&axis_text)
                    .ok()
                    .and_then(|l| axes.iter().position(|a| a.name == l))
                    .ok_or_else(|| MachineDefError::UnknownWorkOffsetAxis {
                        id,
                        axis: axis_text.clone(),
                    })?;
                if !value.is_finite() {
                    return Err(MachineDefError::InvalidWorkOffsetValue {
                        id,
                        axis: axes.get(index).map_or('?', |a| a.name),
                    });
                }
                if let Some(slot) = pos.get_mut(index) {
                    *slot = value;
                }
            }
            work_offsets.insert(WorkOffsetId(id), pos);
        }

        Ok(Self {
            name: raw.machine.name,
            family: raw.machine.family,
            axes,
            speeds,
            work_offsets,
        })
    }

    /// Index of the axis programmed with `letter`.
    pub fn axis_index(&self, letter: char) -> Option<usize> {
        self.axes.iter().position(|a| a.name == letter)
    }

    /// The machine-coordinate origin of a work coordinate system.
    ///
    /// [`WorkOffsetId`] 0 means machine coordinates and is always the zero
    /// position. Other ids not defined in the file are `None`.
    pub fn work_offset(&self, id: WorkOffsetId) -> Option<Pos> {
        if id == WorkOffsetId(0) {
            Some([0.0; MAX_AXES])
        } else {
            self.work_offsets.get(&id).copied()
        }
    }

    /// Ids of the work offsets defined in the file, in order.
    pub fn work_offset_ids(&self) -> impl Iterator<Item = WorkOffsetId> + '_ {
        self.work_offsets.keys().copied()
    }

    /// Checks a machine position against every axis's travel limits.
    ///
    /// Values within 1e-9 of a limit count as inside it, so rounding error
    /// does not produce false alarms. Axes without limits are never out of
    /// range.
    pub fn check_limits(&self, pos: &Pos) -> Vec<LimitViolation> {
        const EPS: f64 = 1e-9;
        self.axes
            .iter()
            .zip(pos)
            .enumerate()
            .filter_map(|(axis, (def, &value))| {
                let (min, max) = def.travel?;
                (value < min - EPS || value > max + EPS || value.is_nan()).then_some(
                    LimitViolation {
                        axis,
                        value,
                        min,
                        max,
                    },
                )
            })
            .collect()
    }
}

/// A position outside an axis's travel limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitViolation {
    /// Index of the axis.
    pub axis: usize,
    /// The position reached.
    pub value: f64,
    /// The axis's lower limit.
    pub min: f64,
    /// The axis's upper limit.
    pub max: f64,
}
