//! Power-on modal state uses Fanuc factory defaults.

mod common;

use dialect_api::{CodeKey, Dialect, DistanceMode, FeedMode, SpindleState};
use dialect_fanuc::Fanuc;
use ir::{ArcPlane, LengthUnit, WorkOffsetId};

#[test]
fn power_on_defaults() {
    let fanuc = Fanuc::new().unwrap();
    let state = fanuc.initial_state(&common::xyz());
    let key = |t| CodeKey::parse(t).unwrap();
    assert_eq!(state.active_code(1), Some(key("G0")));
    assert_eq!(state.active_code(2), Some(key("G17")));
    assert_eq!(state.active_code(3), Some(key("G90")));
    assert_eq!(state.active_code(6), Some(key("G21")));
    assert_eq!(state.active_code(9), Some(key("G80")));
    assert_eq!(state.active_code(14), Some(key("G54")));
    assert_eq!(state.units, LengthUnit::Metric);
    assert_eq!(state.distance, DistanceMode::Absolute);
    assert_eq!(
        state.plane,
        Some(ArcPlane {
            first: 0,
            second: 1
        })
    );
    assert_eq!(state.feed_mode, FeedMode::PerMinute);
    assert_eq!(state.feed_rate, None);
    assert_eq!(state.spindle_state, SpindleState::Off);
    assert_eq!(state.work_offset, WorkOffsetId(1));
}

#[test]
fn table_is_named_and_loads() {
    let fanuc = Fanuc::new().unwrap();
    assert_eq!(fanuc.id(), "fanuc");
    assert!(fanuc.code_table().len() > 50);
}

#[test]
fn machine_without_xy_has_no_initial_plane() {
    let fanuc = Fanuc::new().unwrap();
    let state = fanuc.initial_state(&common::axes(&[
        ('X', Some('I'), false),
        ('Z', Some('K'), false),
    ]));
    assert_eq!(state.plane, None);
}
