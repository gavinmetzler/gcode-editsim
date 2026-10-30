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
        (SegKind::Stop, 7),
        (SegKind::OptionalStop, 8),
        (SegKind::SpindleCw, 9),
        (SegKind::SpindleCcw, 10),
        (SegKind::SpindleStop, 11),
        (SegKind::CoolantFlood, 12),
        (SegKind::CoolantMist, 13),
        (SegKind::CoolantOff, 14),
        (SegKind::ProgramEnd, 15),
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
    for k in [
        SegKind::Dwell,
        SegKind::ToolChange,
        SegKind::Sync,
        SegKind::Stop,
        SegKind::OptionalStop,
        SegKind::SpindleCw,
        SegKind::SpindleCcw,
        SegKind::SpindleStop,
        SegKind::CoolantFlood,
        SegKind::CoolantMist,
        SegKind::CoolantOff,
        SegKind::ProgramEnd,
    ] {
        assert!(!k.is_motion() && !k.is_arc() && k.is_event());
    }
    for k in [
        SegKind::Rapid,
        SegKind::Linear,
        SegKind::ArcCw,
        SegKind::ArcCcw,
    ] {
        assert!(!k.is_event());
    }
}

#[test]
fn seg_flag_bits_are_stable() {
    // The low byte is stored in the render buffer; must not change.
    assert_eq!(SegFlags::COMP_ACTIVE.0, 1);
    assert_eq!(SegFlags::OUT_OF_LIMITS.0, 2);
    assert_eq!(SegFlags::POSITION_UNKNOWN.0, 4);
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

#[test]
fn severity_ids_are_stable() {
    // Stored in the render buffer; must not change.
    assert_eq!(ir::Severity::Info.id(), 0);
    assert_eq!(ir::Severity::Warning.id(), 1);
    assert_eq!(ir::Severity::Error.id(), 2);
}
