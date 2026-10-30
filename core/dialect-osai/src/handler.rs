//! The routines a code-table entry can name.

/// What a code does, as named by `handler` in the code table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Handler {
    Rapid,
    Linear,
    ArcCw,
    ArcCcw,
    /// G17/G18/G19: the plane of two of the first three axes, by position
    /// in the machine's axis list (0-based), first axis first.
    Plane(u8, u8),
    UnitsInch,
    UnitsMetric,
    Absolute,
    Incremental,
    /// G79: the block's coordinates refer to the home position.
    MachineCoords,
    /// G04: dwell at the end of the block.
    Dwell,
    FeedPerMinute,
    /// Changes only feed, cornering or acceleration, or cancels something
    /// that is never active here.
    NoEffect,
    /// Accepted with a warning; the path is shown without its effect.
    NotSimulated,
    /// Not supported, and changes where later blocks move.
    Blocker,
    /// Ends the blocker of its modal group.
    CancelBlocker,
    Stop,
    OptionalStop,
    ProgramEnd,
    ProgramEndRewind,
    SpindleCw,
    SpindleCcw,
    SpindleStop,
    /// M19: spindle stop with angular orientation.
    SpindleOrient,
    ToolChange,
    CoolantAuxiliary,
    CoolantMain,
    CoolantOff,
    SpindleCwCoolant,
    SpindleCcwCoolant,
}

impl Handler {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "rapid" => Self::Rapid,
            "linear" => Self::Linear,
            "arc_cw" => Self::ArcCw,
            "arc_ccw" => Self::ArcCcw,
            "plane_12" => Self::Plane(0, 1),
            "plane_31" => Self::Plane(2, 0),
            "plane_23" => Self::Plane(1, 2),
            "units_inch" => Self::UnitsInch,
            "units_metric" => Self::UnitsMetric,
            "absolute" => Self::Absolute,
            "incremental" => Self::Incremental,
            "machine_coords" => Self::MachineCoords,
            "dwell" => Self::Dwell,
            "feed_per_minute" => Self::FeedPerMinute,
            "no_effect" => Self::NoEffect,
            "not_simulated" => Self::NotSimulated,
            "blocker" => Self::Blocker,
            "cancel_blocker" => Self::CancelBlocker,
            "stop" => Self::Stop,
            "optional_stop" => Self::OptionalStop,
            "program_end" => Self::ProgramEnd,
            "program_end_rewind" => Self::ProgramEndRewind,
            "spindle_cw" => Self::SpindleCw,
            "spindle_ccw" => Self::SpindleCcw,
            "spindle_stop" => Self::SpindleStop,
            "spindle_orient" => Self::SpindleOrient,
            "tool_change" => Self::ToolChange,
            "coolant_auxiliary" => Self::CoolantAuxiliary,
            "coolant_main" => Self::CoolantMain,
            "coolant_off" => Self::CoolantOff,
            "spindle_cw_coolant" => Self::SpindleCwCoolant,
            "spindle_ccw_coolant" => Self::SpindleCcwCoolant,
            _ => return None,
        })
    }

    pub(crate) fn is_arc(self) -> bool {
        matches!(self, Self::ArcCw | Self::ArcCcw)
    }

    /// M codes that take effect before the block's motion (manual chapter
    /// 6: M3, M4, M6, M7, M8, M9 and M19 "before any axis move"; M13 and
    /// M14 are taken to act like M3/M4 (unverified)). The others (M0, M1,
    /// M2, M5, M30) act after it.
    pub(crate) fn before_motion(self) -> bool {
        matches!(
            self,
            Self::SpindleCw
                | Self::SpindleCcw
                | Self::ToolChange
                | Self::CoolantAuxiliary
                | Self::CoolantMain
                | Self::CoolantOff
                | Self::SpindleOrient
                | Self::SpindleCwCoolant
                | Self::SpindleCcwCoolant
        )
    }
}
