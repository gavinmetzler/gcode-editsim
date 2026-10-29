//! Tests for the small value types.

use ir::{CycleStep, SegFlags, SegKind, WorkOffsetId, MAX_AXES};

#[test]
fn seg_kind_values_are_stable() {
    // Stored in the render buffer; must not change.
    let kinds = [
        (SegKind::Rapid, 0u8),
        (SegKind::Linear, 1),
        (SegKind::ArcCw, 2),
        (SegKind::ArcCcw, 3),
        (SegKind::Dwell, 4),
        (SegKind::ToolChange, 5),
        (SegKind::Sync, 6),
    ];
    for (kind, value) in kinds {
        assert_eq!(kind as u8, value);
    }
}

#[test]
fn seg_kind_classification() {
    assert!(SegKind::Rapid.is_motion());
    assert!(SegKind::ArcCw.is_motion() && SegKind::ArcCw.is_arc());
    assert!(!SegKind::Linear.is_arc());
    for k in [SegKind::Dwell, SegKind::ToolChange, SegKind::Sync] {
        assert!(!k.is_motion() && !k.is_arc());
    }
}

#[test]
fn seg_flags_operations() {
    let mut f = SegFlags::EMPTY;
    assert!(f.is_empty());
    f.insert(SegFlags::COMP_ACTIVE);
    assert!(f.contains(SegFlags::COMP_ACTIVE));
    assert!(!f.contains(SegFlags::OUT_OF_LIMITS));
    let both = SegFlags::COMP_ACTIVE | SegFlags::OUT_OF_LIMITS;
    assert!(both.contains(f));
    f.remove(SegFlags::COMP_ACTIVE);
    assert!(f.is_empty());
    // Displayed flags must fit in the render buffer's flag byte.
    assert!(both.0 <= 0xff);
}

#[test]
fn defaults() {
    assert_eq!(CycleStep::default(), CycleStep::None);
    assert_eq!(WorkOffsetId::default(), WorkOffsetId(0));
    assert_eq!(MAX_AXES, 9);
}
