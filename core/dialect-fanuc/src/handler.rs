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
    /// Tool length compensation (G43, G44): never simulated, by design; the
    /// path is shown as programmed, with one note per program.
    LengthCompensation,
    /// Reported as an error.
    Unsupported,
    /// Changes how later blocks move but is not simulated (coordinate
    /// shifts, rotation, scaling, mirroring, polar input, macro and
    /// subprogram calls): a warning, its words are data, and later moves
    /// are drawn without its effect. Moves are never hidden.
    Ignored,
    /// A canned cycle (G73-G89): each hole is drawn as a rapid to its
    /// position in the working plane.
    CannedCycle,
    /// M98: call a subprogram in the same file.
    SubprogramCall,
    /// M99: return from a subprogram.
    SubprogramReturn,
    /// G28: rapid via an intermediate point to the reference position.
    ReferenceReturn,
    /// G30: as G28 but to another reference position, which is not modelled.
    ReferenceReturnOther,
    /// G53: rapid in machine coordinates.
    MachineRapid,
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
            "length_compensation" => Self::LengthCompensation,
            "unsupported" => Self::Unsupported,
            "ignored" => Self::Ignored,
            "canned_cycle" => Self::CannedCycle,
            "subprogram_call" => Self::SubprogramCall,
            "subprogram_return" => Self::SubprogramReturn,
            "reference_return" => Self::ReferenceReturn,
            "reference_return_other" => Self::ReferenceReturnOther,
            "machine_rapid" => Self::MachineRapid,
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
