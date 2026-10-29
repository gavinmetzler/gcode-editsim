//! Travel-limit checks and work offsets.

use ir::{WorkOffsetId, MAX_AXES};
use machine_api::{LimitViolation, MachineDef};

const DEF: &str = r#"
[machine]
name = "m"
family = "mill"

[[axis]]
letter = "X"
kind = "linear"
travel = [-500.0, 0.0]

[[axis]]
letter = "Y"
kind = "linear"

[[axis]]
letter = "Z"
kind = "linear"
travel = [-300.0, 0.0]

[work_offsets]
1 = { X = -250.0, Z = -100.0 }
6 = { Y = 12.5 }
"#;

fn pos(values: &[f64]) -> [f64; MAX_AXES] {
    let mut p = [0.0; MAX_AXES];
    p[..values.len()].copy_from_slice(values);
    p
}

#[test]
fn inside_limits() {
    let def = MachineDef::from_toml_str(DEF).unwrap();
    assert!(def
        .check_limits(&pos(&[-250.0, 99999.0, -300.0]))
        .is_empty());
    // Exactly on a limit, or off by rounding error, is fine.
    assert!(def
        .check_limits(&pos(&[0.0, 0.0, -300.0 - 1e-12]))
        .is_empty());
}

#[test]
fn outside_limits_reports_each_axis() {
    let def = MachineDef::from_toml_str(DEF).unwrap();
    let v = def.check_limits(&pos(&[1.0, 5.0, -301.0]));
    assert_eq!(
        v,
        vec![
            LimitViolation {
                axis: 0,
                value: 1.0,
                min: -500.0,
                max: 0.0
            },
            LimitViolation {
                axis: 2,
                value: -301.0,
                min: -300.0,
                max: 0.0
            },
        ]
    );
}

#[test]
fn nan_position_is_out_of_limits() {
    let def = MachineDef::from_toml_str(DEF).unwrap();
    assert_eq!(def.check_limits(&pos(&[f64::NAN])).len(), 1);
}

#[test]
fn work_offsets() {
    let def = MachineDef::from_toml_str(DEF).unwrap();
    assert_eq!(
        def.work_offset(WorkOffsetId(1)),
        Some(pos(&[-250.0, 0.0, -100.0]))
    );
    assert_eq!(def.work_offset(WorkOffsetId(6)), Some(pos(&[0.0, 12.5])));
    assert_eq!(def.work_offset(WorkOffsetId(2)), None);
    // 0 is machine coordinates.
    assert_eq!(def.work_offset(WorkOffsetId(0)), Some(pos(&[])));
    let ids: Vec<WorkOffsetId> = def.work_offset_ids().collect();
    assert_eq!(ids, vec![WorkOffsetId(1), WorkOffsetId(6)]);
}
