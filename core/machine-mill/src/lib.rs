//! 3-axis milling machine.
//!
//! Three linear axes moving the tool along Cartesian x, y and z, with the
//! tool always pointing along +z. Built from a user's [`MachineDef`]
//! whose `family` is `"mill"`.

#![forbid(unsafe_code)]

use ir::{Pos, WorkOffsetId};
use machine_api::{
    AxisDef, AxisKind, CartesianAxis, LimitViolation, Machine, MachineDef, MachineDefError,
    SpeedLimits, ToolPose,
};
use thiserror::Error;

/// The `family` value this machine type accepts.
pub const FAMILY: &str = "mill";

/// The example definition shipped with this crate
/// (`data/generic_3axis_mill.toml`), as text.
pub const EXAMPLE_DEFINITION: &str = include_str!("../data/generic_3axis_mill.toml");

/// Why a definition is not a valid 3-axis mill.
#[derive(Debug, Error)]
pub enum MillError {
    /// The definition itself could not be loaded.
    #[error(transparent)]
    Definition(#[from] MachineDefError),
    /// The definition is for another machine type.
    #[error("machine family is {0:?}, expected \"mill\"")]
    WrongFamily(String),
    /// Not exactly three axes.
    #[error("a 3-axis mill needs exactly 3 axes, found {0}")]
    AxisCount(usize),
    /// A rotary axis (not supported by this machine type yet).
    #[error("axis {0} is rotary; rotary axes are not supported on a 3-axis mill yet")]
    RotaryAxis(char),
    /// An axis with diameter programming, which only makes sense for turning.
    #[error("axis {0} has diameter mode, which a mill does not support")]
    DiameterMode(char),
    /// An axis whose direction is neither given nor implied by its letter.
    #[error("axis {0}: set direction to \"x\", \"y\" or \"z\"")]
    MissingDirection(char),
    /// Two axes moving in the same Cartesian direction.
    #[error("axes {0} and {1} both move in the same direction")]
    DuplicateDirection(char, char),
}

/// A 3-axis mill.
#[derive(Debug, Clone, PartialEq)]
pub struct Mill {
    def: MachineDef,
    /// For each Cartesian direction (x, y, z), the index of the axis that
    /// moves along it.
    cartesian: [usize; 3],
}

impl Mill {
    /// Checks that `def` describes a 3-axis mill.
    ///
    /// Each axis needs a direction. If none is given, the letters X, Y and
    /// Z imply x, y and z (the standard machine-axis convention); other
    /// letters must say explicitly.
    pub fn new(def: MachineDef) -> Result<Self, MillError> {
        if def.family != FAMILY {
            return Err(MillError::WrongFamily(def.family));
        }
        if def.axes.len() != 3 {
            return Err(MillError::AxisCount(def.axes.len()));
        }
        let mut cartesian: [Option<usize>; 3] = [None; 3];
        for (index, axis) in def.axes.iter().enumerate() {
            if axis.kind == AxisKind::Rotary {
                return Err(MillError::RotaryAxis(axis.name));
            }
            if axis.diameter_mode_capable {
                return Err(MillError::DiameterMode(axis.name));
            }
            let direction = axis
                .direction
                .or(match axis.name {
                    'X' => Some(CartesianAxis::X),
                    'Y' => Some(CartesianAxis::Y),
                    'Z' => Some(CartesianAxis::Z),
                    _ => None,
                })
                .ok_or(MillError::MissingDirection(axis.name))?;
            let slot = cartesian
                .get_mut(direction.index())
                .ok_or(MillError::MissingDirection(axis.name))?;
            if let Some(other) = *slot {
                let other = def.axes.get(other).map_or('?', |a| a.name);
                return Err(MillError::DuplicateDirection(other, axis.name));
            }
            *slot = Some(index);
        }
        // Three axes with three distinct directions fill every slot.
        let [Some(x), Some(y), Some(z)] = cartesian else {
            return Err(MillError::AxisCount(def.axes.len()));
        };
        Ok(Self {
            def,
            cartesian: [x, y, z],
        })
    }

    /// Loads and checks a definition from TOML text.
    pub fn from_toml_str(text: &str) -> Result<Self, MillError> {
        Self::new(MachineDef::from_toml_str(text)?)
    }

    /// The example machine shipped with this crate
    /// (`data/generic_3axis_mill.toml`).
    pub fn generic() -> Result<Self, MillError> {
        Self::from_toml_str(EXAMPLE_DEFINITION)
    }

    /// The definition this mill was built from.
    pub fn definition(&self) -> &MachineDef {
        &self.def
    }
}

impl Machine for Mill {
    fn name(&self) -> &str {
        &self.def.name
    }

    fn axes(&self) -> &[AxisDef] {
        &self.def.axes
    }

    fn forward_kinematics(&self, pos: &Pos) -> ToolPose {
        let [x, y, z] = self.cartesian.map(|i| pos.get(i).copied().unwrap_or(0.0));
        ToolPose {
            position: [x, y, z],
            direction: [0.0, 0.0, 1.0],
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
}
