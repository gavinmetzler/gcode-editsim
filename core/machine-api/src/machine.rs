//! The machine trait.

use ir::{Pos, WorkOffsetId};

use crate::def::{AxisDef, LimitViolation, SpeedLimits};

/// Where the tool tip is and which way the tool points, in the machine's
/// Cartesian space (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolPose {
    /// Tool tip position `[x, y, z]`.
    pub position: [f64; 3],
    /// Unit vector along the tool, pointing from the tip toward the
    /// spindle.
    pub direction: [f64; 3],
}

/// A machine type's model of a physical machine.
///
/// Everything here works in machine coordinates indexed by the machine's
/// axis list, in internal units (mm, degrees).
pub trait Machine {
    /// Display name.
    fn name(&self) -> &str;

    /// The axes, in [`ir::Pos`] order.
    fn axes(&self) -> &[AxisDef];

    /// Number of independent execution channels.
    fn channel_count(&self) -> usize {
        1
    }

    /// Converts machine-axis coordinates to the tool's Cartesian pose.
    fn forward_kinematics(&self, pos: &Pos) -> ToolPose;

    /// Axes of `pos` outside their travel limits (empty if none).
    fn check_limits(&self, pos: &Pos) -> Vec<LimitViolation>;

    /// Machine-coordinate origin of a work coordinate system, or `None` if
    /// the machine definition does not set it.
    fn work_offset(&self, id: WorkOffsetId) -> Option<Pos>;

    /// Machine-wide speed limits.
    fn speeds(&self) -> &SpeedLimits;

    /// Converts a programmed value for `axis` to the machine's convention,
    /// e.g. diameter to radius on turning axes. The default leaves values
    /// unchanged.
    fn normalize_axis_value(&self, axis: usize, value: f64) -> f64 {
        let _ = axis;
        value
    }
}
