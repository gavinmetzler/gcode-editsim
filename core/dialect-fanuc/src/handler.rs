//! The routines a code-table entry can name.

/// What a code does, as named by `handler` in the code table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Handler {
    Rapid,
    Linear,
    ArcCw,
    ArcCcw,
    PlaneXy,
    PlaneZx,
    PlaneYz,
    UnitsInch,
    UnitsMetric,
    Absolute,
    Incremental,
    WorkOffset,
    FeedPerMinute,
    /// Cancels something that is never active here, so it cannot change
    /// the path.
    NoEffect,
    /// Accepted with a warning; the path is shown without its effect.
    NotSimulated,
    /// Reported as an error.
    Unsupported,
    Stop,
    OptionalStop,
    ProgramEnd,
    ProgramEndRewind,
    SpindleCw,
    SpindleCcw,
    SpindleStop,
    ToolChange,
    CoolantFlood,
    CoolantOff,
}

impl Handler {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "rapid" => Self::Rapid,
            "linear" => Self::Linear,
            "arc_cw" => Self::ArcCw,
            "arc_ccw" => Self::ArcCcw,
            "plane_xy" => Self::PlaneXy,
            "plane_zx" => Self::PlaneZx,
            "plane_yz" => Self::PlaneYz,
            "units_inch" => Self::UnitsInch,
            "units_metric" => Self::UnitsMetric,
            "absolute" => Self::Absolute,
            "incremental" => Self::Incremental,
            "work_offset" => Self::WorkOffset,
            "feed_per_minute" => Self::FeedPerMinute,
            "no_effect" => Self::NoEffect,
            "not_simulated" => Self::NotSimulated,
            "unsupported" => Self::Unsupported,
            "stop" => Self::Stop,
            "optional_stop" => Self::OptionalStop,
            "program_end" => Self::ProgramEnd,
            "program_end_rewind" => Self::ProgramEndRewind,
            "spindle_cw" => Self::SpindleCw,
            "spindle_ccw" => Self::SpindleCcw,
            "spindle_stop" => Self::SpindleStop,
            "tool_change" => Self::ToolChange,
            "coolant_flood" => Self::CoolantFlood,
            "coolant_off" => Self::CoolantOff,
            _ => return None,
        })
    }

    pub(crate) fn is_arc(self) -> bool {
        matches!(self, Self::ArcCw | Self::ArcCcw)
    }

    /// M codes that take effect before the block's motion.
    pub(crate) fn before_motion(self) -> bool {
        matches!(
            self,
            Self::SpindleCw | Self::SpindleCcw | Self::CoolantFlood | Self::ToolChange
        )
    }
}
