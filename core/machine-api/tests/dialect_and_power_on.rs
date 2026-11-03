//! M5.6b: a machine file names the dialect its control speaks and the
//! control's power-on state, so choosing the machine sets the dialect and
//! programs that rely on the power-on modes are not warned about them.

use ir::LengthUnit;
use machine_api::{MachineDef, MachineDefError, PowerOnDef, PowerOnDistance, PowerOnMotion};

const AXES: &str = r#"
[[axis]]
letter = "X"
kind = "linear"
[[axis]]
letter = "Y"
kind = "linear"
[[axis]]
letter = "Z"
kind = "linear"
[[axis]]
letter = "A"
kind = "rotary"
"#;

fn load(machine_extra: &str, sections: &str) -> Result<MachineDef, MachineDefError> {
    MachineDef::from_toml_str(&format!(
        "[machine]\nname = \"m\"\nfamily = \"mill\"\n{machine_extra}\n{sections}\n{AXES}"
    ))
}

#[test]
fn the_dialect_is_read_in_lower_case_and_is_optional() {
    assert_eq!(
        load("dialect = \"Osai\"", "")
            .expect("loads")
            .dialect
            .as_deref(),
        Some("osai")
    );
    assert_eq!(load("", "").expect("loads").dialect, None);
}

#[test]
fn an_empty_dialect_is_refused() {
    assert!(matches!(
        load("dialect = \" \"", ""),
        Err(MachineDefError::InvalidDialect(_))
    ));
}

#[test]
fn the_power_on_state_is_read() {
    let def = load(
        "",
        "[power_on]\nunits = \"mm\"\nplane = [\"X\", \"Y\"]\nmotion = \"rapid\"\ndistance = \"absolute\"",
    )
    .expect("loads");
    assert_eq!(
        def.power_on,
        PowerOnDef {
            units: Some(LengthUnit::Metric),
            plane: Some(('X', 'Y')),
            motion: Some(PowerOnMotion::Rapid),
            distance: Some(PowerOnDistance::Absolute),
        }
    );
}

#[test]
fn every_power_on_setting_is_optional() {
    assert_eq!(load("", "").expect("loads").power_on, PowerOnDef::default());
    let def = load(
        "",
        "[power_on]\nunits = \"inch\"\nmotion = \"linear\"\ndistance = \"incremental\"",
    )
    .expect("loads");
    assert_eq!(def.power_on.units, Some(LengthUnit::Inch));
    assert_eq!(def.power_on.plane, None);
    assert_eq!(def.power_on.motion, Some(PowerOnMotion::Linear));
    assert_eq!(def.power_on.distance, Some(PowerOnDistance::Incremental));
}

#[test]
fn the_plane_is_two_different_linear_axes_of_the_machine() {
    for bad in [
        r#"["X", "Q"]"#,
        r#"["X", "X"]"#,
        r#"["X", "A"]"#,
        r#"["X"]"#,
        r#"["X", "Y", "Z"]"#,
    ] {
        let r = load("", &format!("[power_on]\nplane = {bad}"));
        assert!(
            matches!(r, Err(MachineDefError::InvalidPowerOnPlane(_))),
            "{bad}: {r:?}"
        );
    }
    assert_eq!(
        load("", "[power_on]\nplane = [\"z\", \"x\"]")
            .expect("loads")
            .power_on
            .plane,
        Some(('Z', 'X'))
    );
}

#[test]
fn unknown_power_on_values_and_settings_are_refused() {
    assert!(load("", "[power_on]\nunits = \"furlong\"").is_err());
    assert!(load("", "[power_on]\nmotion = \"warp\"").is_err());
    assert!(load("", "[power_on]\nfeed = \"per minute\"").is_err());
}
