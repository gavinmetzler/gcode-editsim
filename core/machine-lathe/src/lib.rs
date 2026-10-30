//! 2-axis lathe (stub).
//!
//! Two linear axes: one across the spindle axis (Cartesian x, the radius)
//! and one along it (Cartesian z). Built from a user's [`MachineDef`]
//! whose `family` is `"lathe"`.
//!
//! This machine type exists in M2 to check that the machine interfaces
//! work for something other than a mill (ARCHITECTURE.md §12). It does
//! the essentials only: diameter programming on the cross axis, the
//! kinematics and limits. Tool nose radius, tool orientation, Y and C
//! axes, sub-spindles and constant surface speed are not modelled.

#![forbid(unsafe_code)]

use ir::{Pos, WorkOffsetId};
use machine_api::{
    AxisDef, AxisKind, CartesianAxis, LimitViolation, Machine, MachineDef, MachineDefError,
    SpeedLimits, ToolPose,
};
use thiserror::Error;

/// The `family` value this machine type accepts.
pub const FAMILY: &str = "lathe";

/// The example definition shipped with this crate
/// (`data/generic_2axis_lathe.toml`), as text.
pub const EXAMPLE_DEFINITION: &str = include_str!("../data/generic_2axis_lathe.toml");

/// Why a definition is not a valid 2-axis lathe.
#[derive(Debug, Error)]
pub enum LatheError {
    /// The definition itself could not be loaded.
    #[error(transparent)]
    Definition(#[from] MachineDefError),
    /// The definition is for another machine type.
    #[error("machine family is {0:?}, expected \"lathe\"")]
    WrongFamily(String),
    /// Not exactly two axes.
    #[error("a 2-axis lathe needs exactly 2 axes, found {0}")]
    AxisCount(usize),
    /// A rotary axis (not supported by this machine type yet).
    #[error("axis {0} is rotary; rotary axes are not supported on a 2-axis lathe yet")]
    RotaryAxis(char),
    /// An axis moving along Cartesian y.
    #[error("axis {0} moves along y; a 2-axis lathe has only x and z axes")]
    NotTurningPlane(char),
    /// Diameter programming on an axis other than the cross (x) axis.
    #[error("axis {0} has diameter mode, which only the x axis of a lathe supports")]
    DiameterMode(char),
    /// An axis whose direction is neither given nor implied by its letter.
    #[error("axis {0}: set direction to \"x\" or \"z\"")]
    MissingDirection(char),
    /// Two axes moving in the same Cartesian direction.
    #[error("axes {0} and {1} both move in the same direction")]
    DuplicateDirection(char, char),
}

/// A 2-axis lathe.
#[derive(Debug, Clone, PartialEq)]
pub struct Lathe {
    def: MachineDef,
    /// Index of the cross (x, radius) axis.
    x: usize,
    /// Index of the spindle-direction (z) axis.
    z: usize,
}

impl Lathe {
    /// Checks that `def` describes a 2-axis lathe.
    ///
    /// Each axis needs a direction. If none is given, the letters X and Z
    /// imply x and z; other letters must say explicitly. Only the x axis
    /// may use diameter mode.
    pub fn new(def: MachineDef) -> Result<Self, LatheError> {
        if def.family != FAMILY {
            return Err(LatheError::WrongFamily(def.family));
        }
        if def.axes.len() != 2 {
            return Err(LatheError::AxisCount(def.axes.len()));
        }
        let mut x: Option<usize> = None;
        let mut z: Option<usize> = None;
        for (index, axis) in def.axes.iter().enumerate() {
            if axis.kind == AxisKind::Rotary {
                return Err(LatheError::RotaryAxis(axis.name));
            }
            let direction = axis
                .direction
                .or(match axis.name {
                    'X' => Some(CartesianAxis::X),
                    'Y' => Some(CartesianAxis::Y),
                    'Z' => Some(CartesianAxis::Z),
                    _ => None,
                })
                .ok_or(LatheError::MissingDirection(axis.name))?;
            let slot = match direction {
                CartesianAxis::X => &mut x,
                CartesianAxis::Z => &mut z,
                CartesianAxis::Y => return Err(LatheError::NotTurningPlane(axis.name)),
            };
            if direction != CartesianAxis::X && axis.diameter_mode_capable {
                return Err(LatheError::DiameterMode(axis.name));
            }
            if let Some(other) = *slot {
                let other = def.axes.get(other).map_or('?', |a| a.name);
                return Err(LatheError::DuplicateDirection(other, axis.name));
            }
            *slot = Some(index);
        }
        // Two axes with two distinct directions (x and z) fill both slots.
        let (Some(x), Some(z)) = (x, z) else {
            return Err(LatheError::AxisCount(def.axes.len()));
        };
        Ok(Self { def, x, z })
    }

    /// Loads and checks a definition from TOML text.
    pub fn from_toml_str(text: &str) -> Result<Self, LatheError> {
        Self::new(MachineDef::from_toml_str(text)?)
    }

    /// The example machine shipped with this crate
    /// (`data/generic_2axis_lathe.toml`).
    pub fn generic() -> Result<Self, LatheError> {
        Self::from_toml_str(EXAMPLE_DEFINITION)
    }

    /// The definition this lathe was built from.
    pub fn definition(&self) -> &MachineDef {
        &self.def
    }
}

impl Machine for Lathe {
    fn name(&self) -> &str {
        &self.def.name
    }

    fn axes(&self) -> &[AxisDef] {
        &self.def.axes
    }

    /// The tool tip in the plane through the spindle axis: x is the
    /// radius, z runs along the spindle, y is always 0. The tool is taken
    /// to point radially outward (+x), toward its holder.
    fn forward_kinematics(&self, pos: &Pos) -> ToolPose {
        let at = |i: usize| pos.get(i).copied().unwrap_or(0.0);
        ToolPose {
            position: [at(self.x), 0.0, at(self.z)],
            direction: [1.0, 0.0, 0.0],
        }
    }

    fn check_limits(&self, pos: &Pos) -> Vec<LimitViolation> {
        self.def.check_limits(pos)
    }

    fn work_offset(&self, id: WorkOffsetId) -> Option<Pos> {
        self.def.work_offset(id)
    }

    fn speeds(&self) -> &SpeedLimits {
        &self.def.speeds
    }

    /// A diameter-mode axis is programmed as a diameter; the machine
    /// position is the radius, so the value is halved. Other axes are
    /// unchanged.
    fn normalize_axis_value(&self, axis: usize, value: f64) -> f64 {
        match self.def.axes.get(axis) {
            Some(a) if a.diameter_mode_capable => value / 2.0,
            _ => value,
        }
    }
}
