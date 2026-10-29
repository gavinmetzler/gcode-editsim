//! Tests for `AxisSet`.

use dialect_api::{AxisInfo, AxisSet, AxisSetError};
use ir::ArcPlane;

fn axis(letter: char, offset: Option<char>) -> AxisInfo {
    AxisInfo {
        letter,
        arc_offset_letter: offset,
        rotary: false,
    }
}

fn three_axes() -> AxisSet {
    AxisSet::new(vec![
        axis('X', Some('I')),
        axis('Y', Some('J')),
        axis('Z', Some('K')),
    ])
    .unwrap()
}

#[test]
fn lookups_by_letter() {
    let axes = three_axes();
    assert_eq!(axes.len(), 3);
    assert_eq!(axes.index_of('Y'), Some(1));
    assert_eq!(axes.index_of('A'), None);
    assert_eq!(axes.index_of_arc_offset('K'), Some(2));
    assert_eq!(axes.index_of_arc_offset('X'), None);
}

#[test]
fn planes_follow_the_given_order() {
    let axes = three_axes();
    assert_eq!(
        axes.plane('X', 'Y'),
        Some(ArcPlane {
            first: 0,
            second: 1
        })
    );
    assert_eq!(
        axes.plane('Z', 'X'),
        Some(ArcPlane {
            first: 2,
            second: 0
        })
    );
    assert_eq!(axes.plane('X', 'A'), None);
    assert_eq!(axes.plane('X', 'X'), None);
}

#[test]
fn any_letters_work_not_just_xyz() {
    // e.g. a two-axis machine programmed with other letters.
    let axes = AxisSet::new(vec![axis('U', Some('P')), axis('W', None)]).unwrap();
    assert_eq!(axes.index_of('W'), Some(1));
    assert_eq!(axes.index_of('X'), None);
    assert_eq!(
        axes.plane('U', 'W'),
        Some(ArcPlane {
            first: 0,
            second: 1
        })
    );
}

#[test]
fn empty_set_is_allowed() {
    let axes = AxisSet::new(vec![]).unwrap();
    assert!(axes.is_empty());
}

#[test]
fn rejects_too_many_axes() {
    let letters = "ABCDEFGHIJ";
    let many: Vec<AxisInfo> = letters.chars().map(|c| axis(c, None)).collect();
    assert_eq!(AxisSet::new(many), Err(AxisSetError::TooMany(10)));
}

#[test]
fn rejects_duplicate_and_invalid_letters() {
    assert_eq!(
        AxisSet::new(vec![axis('X', None), axis('X', None)]),
        Err(AxisSetError::Duplicate('X'))
    );
    assert_eq!(
        AxisSet::new(vec![axis('X', Some('Y')), axis('Y', None)]),
        Err(AxisSetError::Duplicate('Y'))
    );
    assert_eq!(
        AxisSet::new(vec![axis('x', None)]),
        Err(AxisSetError::InvalidLetter('x'))
    );
    assert_eq!(
        AxisSet::new(vec![axis('X', Some('#'))]),
        Err(AxisSetError::InvalidLetter('#'))
    );
}

#[test]
fn rotary_flag_is_kept() {
    let axes = AxisSet::new(vec![
        axis('X', Some('I')),
        AxisInfo {
            letter: 'A',
            arc_offset_letter: None,
            rotary: true,
        },
    ])
    .unwrap();
    assert!(!axes.axes()[0].rotary);
    assert!(axes.axes()[1].rotary);
}
