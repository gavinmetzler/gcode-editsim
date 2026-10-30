//! Machine trait, axis definitions, kinematics and the machine definition
//! file format.
//!
//! A *machine type* (such as a 3-axis mill) implements [`Machine`]. The
//! specific machine a user owns is described by a [`MachineDef`], loaded
//! from a TOML text file that the user writes (see
//! `docs/machine-definition.md`). The machine type checks that the
//! definition makes sense for it.
//!
//! All values are in internal units: mm, degrees, mm/min, rpm.

#![forbid(unsafe_code)]

mod def;
mod machine;

pub use def::{
    AxisDef, AxisKind, CartesianAxis, LimitViolation, MachineDef, MachineDefError, SpeedLimits,
};
pub use machine::{Machine, ToolPose};
