//! Tests for `ModalState`.

use dialect_api::{CodeKey, ModalState};

#[test]
fn active_codes_per_group() {
    let mut state = ModalState::default();
    assert_eq!(state.active_code(1), None);
    let g1 = CodeKey::parse("G1").unwrap();
    state.set_active(1, g1);
    assert_eq!(state.active_code(1), Some(g1));
}

#[test]
fn first_report_is_true_only_once_per_key() {
    let mut state = ModalState::default();
    assert!(state.first_report("decimal"));
    assert!(!state.first_report("decimal"));
    assert!(state.first_report("other"));
}

#[test]
fn motion_blockers_start_and_end() {
    let mut state = ModalState::default();
    assert_eq!(state.motion_blocker(), None);
    assert!(state.block_motion("G68", 4));
    assert!(!state.block_motion("G68", 9));
    assert_eq!(state.motion_blocker(), Some(("G68", 4)));
    assert_eq!(state.unblock_motion("G68"), Some(4));
    assert_eq!(state.unblock_motion("G68"), None);
    assert_eq!(state.motion_blocker(), None);
}
