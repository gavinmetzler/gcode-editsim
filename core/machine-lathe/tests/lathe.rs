//! Tests for the lathe machine type (stub).

use ir::{WorkOffsetId, MAX_AXES};
use machine_api::Machine;
use machine_lathe::{Lathe, LatheError};

fn pos(values: &[f64]) -> [f64; MAX_AXES] {
    let mut p = [0.0; MAX_AXES];
    p[..values.len()].copy_from_slice(values);
    p
}

fn lathe(axes: &str) -> Result<Lathe, LatheError> {
    Lathe::from_toml_str(&format!(
        "[machine]\nname = \"l\"\nfamily = \"lathe\"\n{axes}"
    ))
}

fn axis(letter: &str, extra: &str) -> String {
    format!("[[axis]]\nletter = \"{letter}\"\nkind = \"linear\"\n{extra}\n")
}

#[test]
fn shipped_example_loads() {
    let l = Lathe::generic().unwrap();
    assert_eq!(l.name(), "Generic 2-axis lathe");
    assert_eq!(l.axes().len(), 2);
    assert_eq!(l.channel_count(), 1);
    assert!(l.axes()[0].diameter_mode_capable);
    assert!(!l.axes()[1].diameter_mode_capable);
    assert_eq!(l.work_offset(WorkOffsetId(1)), Some(pos(&[0.0, 100.0])));
}

#[test]
fn diameter_axis_values_are_halved() {
    let l = Lathe::generic().unwrap();
    assert_eq!(l.normalize_axis_value(0, 50.0), 25.0);
    assert_eq!(l.normalize_axis_value(0, -3.0), -1.5);
    // Z is not a diameter axis.
    assert_eq!(l.normalize_axis_value(1, 50.0), 50.0);
    // Out-of-range axis indices are left alone, not a panic.
    assert_eq!(l.normalize_axis_value(7, 50.0), 50.0);
    assert_eq!(l.normalize_axis_value(usize::MAX, 50.0), 50.0);
}

#[test]
fn radius_programming_when_diameter_mode_is_off() {
    let l = lathe(&(axis("X", "") + &axis("Z", ""))).unwrap();
    assert_eq!(l.normalize_axis_value(0, 50.0), 50.0);
}

#[test]
fn kinematics_maps_x_to_radius_and_z_along_the_spindle() {
    let l = Lathe::generic().unwrap();
    let pose = l.forward_kinematics(&pos(&[25.0, -40.0]));
    assert_eq!(pose.position, [25.0, 0.0, -40.0]);
    assert_eq!(pose.direction, [1.0, 0.0, 0.0]);
}

#[test]
fn kinematics_follows_directions_not_list_order() {
    let l = lathe(&(axis("Z", "") + &axis("X", ""))).unwrap();
    assert_eq!(
        l.forward_kinematics(&pos(&[-40.0, 25.0])).position,
        [25.0, 0.0, -40.0]
    );
    // Diameter mode follows the axis, wherever it is in the list.
    let l = lathe(&(axis("Z", "") + &axis("X", "diameter_mode = true"))).unwrap();
    assert_eq!(l.normalize_axis_value(1, 10.0), 5.0);
    assert_eq!(l.normalize_axis_value(0, 10.0), 10.0);
}

#[test]
fn other_letters_need_explicit_directions() {
    let l = lathe(&(axis("U", "direction = \"x\"") + &axis("W", "direction = \"z\""))).unwrap();
    assert_eq!(
        l.forward_kinematics(&pos(&[4.0, 6.0])).position,
        [4.0, 0.0, 6.0]
    );
    assert!(matches!(
        lathe(&(axis("U", "") + &axis("Z", ""))),
        Err(LatheError::MissingDirection('U'))
    ));
}

#[test]
fn limits_are_in_radius() {
    let l = Lathe::generic().unwrap();
    // 150 radius (300 diameter) is the edge of X travel.
    assert!(l.check_limits(&pos(&[150.0, 0.0])).is_empty());
    assert_eq!(l.check_limits(&pos(&[150.5, 0.0])).len(), 1);
}

#[test]
fn rejects_wrong_family() {
    let text = format!(
        "[machine]\nname = \"m\"\nfamily = \"mill\"\n{}{}",
        axis("X", ""),
        axis("Z", "")
    );
    assert!(matches!(
        Lathe::from_toml_str(&text),
        Err(LatheError::WrongFamily(f)) if f == "mill"
    ));
}

#[test]
fn rejects_wrong_axis_count() {
    assert!(matches!(
        lathe(&axis("X", "")),
        Err(LatheError::AxisCount(1))
    ));
    assert!(matches!(
        lathe(&(axis("X", "") + &axis("Y", "") + &axis("Z", ""))),
        Err(LatheError::AxisCount(3))
    ));
}

#[test]
fn rejects_a_y_axis() {
    assert!(matches!(
        lathe(&(axis("X", "") + &axis("Y", ""))),
        Err(LatheError::NotTurningPlane('Y'))
    ));
}

#[test]
fn rejects_two_axes_in_one_direction() {
    assert!(matches!(
        lathe(&(axis("X", "") + &axis("U", "direction = \"x\""))),
        Err(LatheError::DuplicateDirection('X', 'U'))
    ));
}

#[test]
fn rejects_rotary_axes() {
    let c = "[[axis]]\nletter = \"C\"\nkind = \"rotary\"\ndirection = \"z\"\n";
    assert!(matches!(
        lathe(&(axis("X", "") + c)),
        Err(LatheError::RotaryAxis('C'))
    ));
}

#[test]
fn diameter_mode_only_on_the_x_axis() {
    assert!(matches!(
        lathe(&(axis("X", "") + &axis("Z", "diameter_mode = true"))),
        Err(LatheError::DiameterMode('Z'))
    ));
}

#[test]
fn bad_definition_is_an_error_not_a_panic() {
    assert!(matches!(
        Lathe::from_toml_str("not toml ["),
        Err(LatheError::Definition(_))
    ));
}
