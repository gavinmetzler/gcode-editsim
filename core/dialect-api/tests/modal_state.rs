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
