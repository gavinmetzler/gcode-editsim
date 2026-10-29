//! G90/G91 persist across blocks; arc-centre offsets are always relative.

mod common;

use common::{inc, run, w};
use dialect_api::{ArcCenter, AxisOffset, Command, DistanceMode};

#[test]
fn g91_persists_until_g90() {
    let r = run("G91\nG0 X1.\nY2.\nG90\nX3.\n");
    assert!(r
        .commands
        .contains(&Command::SetDistanceMode(DistanceMode::Incremental)));
    assert_eq!(
        r.moves(),
        vec![
            &Command::Rapid {
                target: vec![inc(0, 1.0)]
            },
            &Command::Rapid {
                target: vec![inc(1, 2.0)]
            },
            &Command::Rapid {
                target: vec![w(0, 3.0)]
            },
        ]
    );
}

#[test]
fn mode_change_applies_to_its_own_block() {
    let r = run("G91 G0 X1.\n");
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![inc(0, 1.0)]
        }]
    );
}

#[test]
fn arc_offsets_are_relative_in_either_mode() {
    for prefix in ["G90", "G91"] {
        let r = run(&format!("{prefix} G2 X10. Y0 I5. J0 F100.\n"));
        let Some(Command::Arc { center, .. }) = r.moves().first().copied() else {
            panic!("no arc for {prefix}");
        };
        assert_eq!(
            center,
            &ArcCenter::Offset(vec![
                AxisOffset {
                    axis: 0,
                    value: 5.0
                },
                AxisOffset {
                    axis: 1,
                    value: 0.0
                }
            ])
        );
    }
}
