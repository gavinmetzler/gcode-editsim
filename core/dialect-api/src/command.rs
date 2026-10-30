//! Commands: machine-neutral intents produced by a dialect.

use ir::{ArcPlane, LengthUnit, WorkOffsetId};

use crate::state::{CoolantState, DistanceMode, FeedMode, SpindleState};

/// A programmed value for one axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisWord {
    /// Index into the machine's [`AxisSet`](crate::AxisSet).
    pub axis: usize,
    /// Value in internal units (mm or degrees), already converted from the
    /// program's units.
    pub value: f64,
    /// `true` if `value` is a distance from the current position rather
    /// than an absolute coordinate.
    pub incremental: bool,
}

/// Direction of an arc, following the orientation of its [`ArcPlane`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArcDir {
    /// Clockwise.
    Cw,
    /// Counter-clockwise.
    Ccw,
}

/// An offset along one axis, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisOffset {
    /// Index into the machine's [`AxisSet`](crate::AxisSet).
    pub axis: usize,
    /// Offset in mm.
    pub value: f64,
}

/// How an arc's centre was programmed.
#[derive(Debug, Clone, PartialEq)]
pub enum ArcCenter {
    /// Offsets from the arc's start point to its centre. Axes not listed
    /// have a zero offset.
    Offset(Vec<AxisOffset>),
    /// Radius in mm. Negative selects the arc longer than half a circle.
    Radius(f64),
    /// The centre as a point, programmed like an end point: an absolute
    /// word is a coordinate in the active work coordinate system, an
    /// incremental word a distance from the arc's start. Values are
    /// converted by the machine like axis words (e.g. diameter to radius).
    /// A plane axis not listed takes the start's coordinate.
    Position(Vec<AxisWord>),
}

/// One machine-neutral intent. A block may produce several, in the order
/// they take effect.
///
/// Commands carry no source location; whoever calls the dialect attaches
/// the block's [`ir::SourceRef`] to everything it produced.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Move at rapid rate.
    Rapid {
        /// Programmed axes; unlisted axes do not move.
        target: Vec<AxisWord>,
    },
    /// Move in a straight line at the current feed rate.
    Linear {
        /// Programmed axes; unlisted axes do not move.
        target: Vec<AxisWord>,
    },
    /// Move along an arc at the current feed rate.
    Arc {
        /// End point; unlisted axes end where they started.
        target: Vec<AxisWord>,
        /// Centre.
        center: ArcCenter,
        /// Direction.
        dir: ArcDir,
        /// Plane of rotation.
        plane: ArcPlane,
    },
    /// Return to a reference position, as a rapid via an intermediate
    /// point. Only the listed axes move.
    ReturnToReference {
        /// The intermediate point, programmed like a rapid target. Each
        /// listed axis goes there first, then to the reference position.
        via: Vec<AxisWord>,
        /// Which reference position: 1 is the machine's home position (from
        /// its definition); 2 and up are further positions the machine
        /// definition does not describe yet.
        position: u8,
    },
    /// Rapid move to absolute machine coordinates, ignoring work offsets.
    MachineRapid {
        /// Target in machine coordinates (always absolute).
        target: Vec<AxisWord>,
    },
    /// The machine moved these axes somewhere the dialect cannot model, so
    /// their position is unknown until they are next given an absolute
    /// position.
    PositionUnknown {
        /// Indices into the machine's [`AxisSet`](crate::AxisSet).
        axes: Vec<usize>,
        /// What made the position unknown, for diagnostics.
        reason: String,
    },
    /// Pause for a fixed time.
    Dwell {
        /// Seconds.
        seconds: f64,
    },
    /// Set the feed rate, in mm per minute (or per revolution, or inverse
    /// time, according to the active [`FeedMode`]).
    SetFeedRate(f64),
    /// Units the program now uses (values in commands are already in mm).
    SetUnits(LengthUnit),
    /// Absolute or incremental positioning.
    SetDistanceMode(DistanceMode),
    /// Plane for subsequent arcs.
    SetPlane(ArcPlane),
    /// Work coordinate system.
    SetWorkOffset(WorkOffsetId),
    /// How the feed rate is interpreted.
    SetFeedMode(FeedMode),
    /// Spindle speed in rpm. Does not start or stop the spindle.
    SetSpindleSpeed {
        /// Spindle number, 0 for the main spindle.
        spindle: u8,
        /// Speed in rpm.
        rpm: f64,
    },
    /// Start or stop a spindle.
    SetSpindleState {
        /// Spindle number, 0 for the main spindle.
        spindle: u8,
        /// New state.
        state: SpindleState,
    },
    /// Choose the next tool (does not change it yet).
    SelectTool(u32),
    /// Change to a tool now.
    ToolChange {
        /// Tool number.
        tool: u32,
    },
    /// Coolant on or off.
    Coolant(CoolantState),
    /// Program stop that waits for the operator.
    Stop {
        /// `true` if the stop only happens when the operator has enabled
        /// optional stops.
        optional: bool,
    },
    /// End of program.
    ProgramEnd {
        /// `true` if the control rewinds to the start of the program.
        rewind: bool,
    },
    /// Something the dialect recognised but cannot express yet. Always
    /// accompanied by a diagnostic.
    Unsupported {
        /// Short description, e.g. the code that was skipped.
        what: String,
    },
}
