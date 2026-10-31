//! Tests for the 3-axis mill machine type.

use ir::{WorkOffsetId, MAX_AXES};
use machine_api::{Machine, MachineDef};
use machine_mill::{Mill, MillError};

fn pos(values: &[f64]) -> [f64; MAX_AXES] {
    let mut p = [0.0; MAX_AXES];
    p[..values.len()].copy_from_slice(values);
    p
}

fn mill(axes: &str) -> Result<Mill, MillError> {
    Mill::from_toml_str(&format!(
        "[machine]\nname = \"m\"\nfamily = \"mill\"\n{axes}"
    ))
}

fn axis(letter: &str, extra: &str) -> String {
    format!("[[axis]]\nletter = \"{letter}\"\nkind = \"linear\"\n{extra}\n")
}

#[test]
fn shipped_example_loads() {
    let m = Mill::generic().unwrap();
    assert_eq!(m.name(), "Generic 3-axis VMC");
    assert_eq!(m.axes().len(), 3);
    assert_eq!(m.channel_count(), 1);
    assert_eq!(m.speeds().max_spindle_rpm, Some(12000.0));
    assert!(m.work_offset(WorkOffsetId(1)).is_some());
    let letters: Vec<Option<char>> = m.axes().iter().map(|a| a.arc_offset_letter).collect();
    assert_eq!(letters, vec![Some('I'), Some('J'), Some('K')]);
}

#[test]
fn kinematics_is_identity_for_xyz() {
    let m = mill(&(axis("X", "") + &axis("Y", "") + &axis("Z", ""))).unwrap();
    let pose = m.forward_kinematics(&pos(&[1.0, 2.0, 3.0]));
    assert_eq!(pose.position, [1.0, 2.0, 3.0]);
    assert_eq!(pose.direction, [0.0, 0.0, 1.0]);
}

#[test]
fn kinematics_follows_axis_directions_not_list_order() {
    // Axes listed Z, X, Y.
    let m = mill(&(axis("Z", "") + &axis("X", "") + &axis("Y", ""))).unwrap();
    let pose = m.forward_kinematics(&pos(&[3.0, 1.0, 2.0]));
    assert_eq!(pose.position, [1.0, 2.0, 3.0]);
}

#[test]
fn other_letters_need_explicit_directions() {
    let m = mill(
        &(axis("U", "direction = \"x\"")
            + &axis("V", "direction = \"y\"")
            + &axis("W", "direction = \"z\"")),
    )
    .unwrap();
    assert_eq!(
        m.forward_kinematics(&pos(&[4.0, 5.0, 6.0])).position,
        [4.0, 5.0, 6.0]
    );

    let e = mill(&(axis("U", "") + &axis("Y", "") + &axis("Z", ""))).unwrap_err();
    assert!(matches!(e, MillError::MissingDirection('U')));
}

#[test]
fn explicit_direction_overrides_the_letter() {
    let m =
        mill(&(axis("X", "direction = \"y\"") + &axis("Y", "direction = \"x\"") + &axis("Z", "")))
            .unwrap();
    assert_eq!(
        m.forward_kinematics(&pos(&[1.0, 2.0, 3.0])).position,
        [2.0, 1.0, 3.0]
    );
}

#[test]
fn rejects_definitions_that_are_not_a_3_axis_mill() {
    let xyz = axis("X", "") + &axis("Y", "") + &axis("Z", "");
    let lathe = format!("[machine]\nname = \"m\"\nfamily = \"lathe\"\n{xyz}");
    assert!(matches!(Mill::from_toml_str(&lathe), Err(MillError::WrongFamily(f)) if f == "lathe"));

    assert!(matches!(
        mill(&(axis("X", "") + &axis("Y", ""))),
        Err(MillError::AxisCount(2))
    ));

    let rotary = axis("X", "") + &axis("Y", "") + "[[axis]]\nletter = \"A\"\nkind = \"rotary\"\n";
    assert!(matches!(mill(&rotary), Err(MillError::RotaryAxis('A'))));

    let diameter = axis("X", "diameter_mode = true") + &axis("Y", "") + &axis("Z", "");
    assert!(matches!(mill(&diameter), Err(MillError::DiameterMode('X'))));

    let twice = axis("X", "") + &axis("Y", "") + &axis("W", "direction = \"x\"");
    assert!(matches!(
        mill(&twice),
        Err(MillError::DuplicateDirection('X', 'W'))
    ));

    assert!(matches!(
        mill("[[axis]]\nletter = \"X\"\nkind = \"what\""),
        Err(MillError::Definition(_))
    ));
}

#[test]
fn limits_and_offsets_come_from_the_definition() {
    let m = Mill::generic().unwrap();
    let def: &MachineDef = m.definition();
    let inside = pos(&[-100.0, -100.0, -100.0]);
    assert!(m.check_limits(&inside).is_empty());
    let outside = pos(&[10.0, -100.0, -100.0]);
    assert_eq!(m.check_limits(&outside).len(), 1);
    assert_eq!(
        m.work_offset(WorkOffsetId(2)),
        def.work_offset(WorkOffsetId(2))
    );
    assert_eq!(m.normalize_axis_value(0, 12.5), 12.5);
}

#[test]
fn works_as_a_trait_object() {
    let m: Box<dyn Machine> = Box::new(Mill::generic().unwrap());
    assert_eq!(m.axes()[2].name, 'Z');
}

#[test]
fn router_example_loads_without_travel_limits() {
    use machine_api::{Machine, MachineDef};
    use machine_mill::{Mill, ROUTER_EXAMPLE_DEFINITION};
    let mill = Mill::new(MachineDef::from_toml_str(ROUTER_EXAMPLE_DEFINITION).unwrap()).unwrap();
    assert!(mill.axes().iter().all(|a| a.travel.is_none()));
    // Far from the origin, nothing is out of limits.
    let mut far = [0.0; ir::MAX_AXES];
    far[0] = 3000.0;
    far[1] = -1500.0;
    assert!(mill.check_limits(&far).is_empty());
    for n in 1..=6 {
        assert_eq!(
            mill.work_offset(ir::WorkOffsetId(n)),
            Some([0.0; ir::MAX_AXES])
        );
    }
}
