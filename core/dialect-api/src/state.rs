//! Modal state carried from block to block.

use std::collections::{BTreeMap, BTreeSet};

use ir::{ArcPlane, LengthUnit, WorkOffsetId};

use crate::code::CodeKey;

/// Absolute or incremental positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DistanceMode {
    /// Axis words are coordinates.
    Absolute,
    /// Axis words are distances from the current position.
    Incremental,
}

/// How feed-rate values are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeedMode {
    /// Distance per minute.
    PerMinute,
    /// Distance per spindle revolution.
    PerRevolution,
    /// The move takes 1/F minutes.
    InverseTime,
}

/// Spindle rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpindleState {
    /// Stopped.
    Off,
    /// Turning clockwise.
    Cw,
    /// Turning counter-clockwise.
    Ccw,
}

/// Coolant state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoolantState {
    /// All coolant off.
    Off,
    /// Flood coolant on.
    Flood,
    /// Mist coolant on.
    Mist,
}

/// Everything a dialect remembers between blocks.
///
/// The machine position is not here: it belongs to the interpreter, which
/// owns the machine model and work offsets.
#[derive(Debug, Clone, PartialEq)]
pub struct ModalState {
    /// Active code in each modal group, keyed by the group number from the
    /// dialect's code table.
    pub active: BTreeMap<u16, CodeKey>,
    /// Units the program is written in.
    pub units: LengthUnit,
    /// Absolute or incremental positioning.
    pub distance: DistanceMode,
    /// Plane for arcs, if one has been selected.
    pub plane: Option<ArcPlane>,
    /// How feed values are interpreted.
    pub feed_mode: FeedMode,
    /// Current feed rate (in the units of `feed_mode`, converted to mm), if
    /// one has been programmed.
    pub feed_rate: Option<f64>,
    /// Programmed spindle speed in rpm.
    pub spindle_rpm: f64,
    /// Spindle rotation.
    pub spindle_state: SpindleState,
    /// Tool chosen for the next tool change, if any.
    pub selected_tool: Option<u32>,
    /// Tool currently in the spindle, if known.
    pub current_tool: Option<u32>,
    /// Active work coordinate system.
    pub work_offset: WorkOffsetId,
    /// Keys of program-wide conditions already reported, so a dialect can
    /// warn about something once per program instead of on every block.
    pub reported_once: BTreeSet<String>,
}

impl Default for ModalState {
    /// A neutral starting state: metric, absolute, no plane, feed per
    /// minute, spindle off, no tool, machine coordinates. Dialects override
    /// this with their own power-on defaults.
    fn default() -> Self {
        Self {
            active: BTreeMap::new(),
            units: LengthUnit::Metric,
            distance: DistanceMode::Absolute,
            plane: None,
            feed_mode: FeedMode::PerMinute,
            feed_rate: None,
            spindle_rpm: 0.0,
            spindle_state: SpindleState::Off,
            selected_tool: None,
            current_tool: None,
            work_offset: WorkOffsetId::default(),
            reported_once: BTreeSet::new(),
        }
    }
}

impl ModalState {
    /// The active code in modal group `group`.
    pub fn active_code(&self, group: u16) -> Option<CodeKey> {
        self.active.get(&group).copied()
    }

    /// Makes `code` the active code of modal group `group`.
    pub fn set_active(&mut self, group: u16, code: CodeKey) {
        self.active.insert(group, code);
    }

    /// Returns `true` the first time it is called with `key` for this
    /// state, and `false` afterwards.
    pub fn first_report(&mut self, key: &str) -> bool {
        self.reported_once.insert(key.to_owned())
    }
}
