//! Loading machine definitions from TOML.

use machine_api::{AxisKind, CartesianAxis, MachineDef, MachineDefError};

const FULL: &str = r#"
[machine]
name = "Test machine"
family = "mill"
max_feed = 8000.0
max_spindle_rpm = 10000.0

[[axis]]
letter = "X"
kind = "linear"
direction = "x"
arc_offset = "I"
travel = [-500.0, 0.0]
rapid = 30000.0

[[axis]]
letter = "z"
kind = "linear"
arc_offset = "k"

[[axis]]
letter = "A"
kind = "rotary"
direction = "x"
rapid = 7200.0

[work_offsets]
1 = { X = -250.0, Z = -300.5 }
3 = { A = 90.0 }
"#;

fn minimal(axes: &str) -> String {
    format!("[machine]\nname = \"m\"\nfamily = \"mill\"\n{axes}")
}

#[test]
fn loads_every_setting() {
    let def = MachineDef::from_toml_str(FULL).unwrap();
    assert_eq!(def.name, "Test machine");
    assert_eq!(def.family, "mill");
    assert_eq!(def.speeds.max_feed, Some(8000.0));
    assert_eq!(def.speeds.max_spindle_rpm, Some(10000.0));
    assert_eq!(def.axes.len(), 3);

    let x = &def.axes[0];
    assert_eq!(x.name, 'X');
    assert_eq!(x.kind, AxisKind::Linear);
    assert_eq!(x.direction, Some(CartesianAxis::X));
    assert_eq!(x.arc_offset_letter, Some('I'));
    assert_eq!(x.travel, Some((-500.0, 0.0)));
    assert_eq!(x.rapid_rate, Some(30000.0));
    assert!(!x.diameter_mode_capable);

    // Lower-case letters are accepted and upper-cased; optional settings
    // default to "not given".
    let z = &def.axes[1];
    assert_eq!(z.name, 'Z');
    assert_eq!(z.arc_offset_letter, Some('K'));
    assert_eq!(z.travel, None);
    assert_eq!(z.direction, None);
    assert_eq!(z.rapid_rate, None);

    assert_eq!(def.axes[2].kind, AxisKind::Rotary);
    assert_eq!(def.axis_index('A'), Some(2));
    assert_eq!(def.axis_index('Y'), None);
}

#[test]
fn speeds_are_optional() {
    let def = MachineDef::from_toml_str(&minimal("")).unwrap();
    assert!(def.axes.is_empty());
    assert_eq!(def.speeds.max_feed, None);
}

fn err(text: &str) -> MachineDefError {
    MachineDef::from_toml_str(text).unwrap_err()
}

#[test]
fn rejects_bad_toml_missing_and_unknown_settings() {
    assert!(matches!(err("not toml ["), MachineDefError::Toml(_)));
    assert!(matches!(
        err("[machine]\nname = \"m\""),
        MachineDefError::Toml(_)
    ));
    assert!(matches!(
        err(&minimal(
            "[[axis]]\nletter = \"X\"\nkind = \"linear\"\ntravle = [0.0, 1.0]"
        )),
        MachineDefError::Toml(_)
    ));
    assert!(matches!(
        err(&minimal("[[axis]]\nletter = \"X\"\nkind = \"sideways\"")),
        MachineDefError::Toml(_)
    ));
    assert!(matches!(
        err(&minimal(
            "[[axis]]\nletter = \"X\"\nkind = \"linear\"\ndirection = \"w\""
        )),
        MachineDefError::Toml(_)
    ));
}

#[test]
fn rejects_too_many_axes() {
    let axes: String = "ABCDEFGHJK"
        .chars()
        .map(|c| format!("[[axis]]\nletter = \"{c}\"\nkind = \"linear\"\n"))
        .collect();
    assert!(matches!(
        err(&minimal(&axes)),
        MachineDefError::TooManyAxes(10)
    ));
}

#[test]
fn rejects_bad_and_duplicate_letters() {
    for bad in ["XY", "", "1", "#"] {
        let e = err(&minimal(&format!(
            "[[axis]]\nletter = \"{bad}\"\nkind = \"linear\""
        )));
        assert!(matches!(e, MachineDefError::InvalidLetter(_)), "{bad}");
    }
    let dup =
        "[[axis]]\nletter = \"X\"\nkind = \"linear\"\n[[axis]]\nletter = \"x\"\nkind = \"linear\"";
    assert!(matches!(
        err(&minimal(dup)),
        MachineDefError::DuplicateLetter('X')
    ));
    let clash = "[[axis]]\nletter = \"X\"\nkind = \"linear\"\narc_offset = \"Y\"\n[[axis]]\nletter = \"Y\"\nkind = \"linear\"";
    assert!(matches!(
        err(&minimal(clash)),
        MachineDefError::DuplicateLetter('Y')
    ));
}

#[test]
fn rejects_bad_travel() {
    for travel in ["[0.0, 0.0]", "[5.0, -5.0]", "[0.0, inf]", "[nan, 1.0]"] {
        let e = err(&minimal(&format!(
            "[[axis]]\nletter = \"X\"\nkind = \"linear\"\ntravel = {travel}"
        )));
        assert!(
            matches!(e, MachineDefError::InvalidTravel { axis: 'X' }),
            "{travel}"
        );
    }
}

#[test]
fn rejects_bad_speeds() {
    for (setting, value) in [
        ("max_feed", "0.0"),
        ("max_spindle_rpm", "-1.0"),
        ("max_feed", "inf"),
    ] {
        let text = format!("[machine]\nname = \"m\"\nfamily = \"mill\"\n{setting} = {value}");
        assert!(
            matches!(err(&text), MachineDefError::InvalidSpeed { .. }),
            "{setting}"
        );
    }
    let rapid = minimal("[[axis]]\nletter = \"X\"\nkind = \"linear\"\nrapid = 0.0");
    let e = err(&rapid);
    assert_eq!(
        e.to_string(),
        "axis X: rapid must be a number greater than zero"
    );
}

#[test]
fn rejects_arc_offset_on_rotary_axis() {
    let e = err(&minimal(
        "[[axis]]\nletter = \"A\"\nkind = \"rotary\"\narc_offset = \"I\"",
    ));
    assert!(matches!(e, MachineDefError::ArcOffsetOnRotary('A')));
}

#[test]
fn rejects_bad_work_offsets() {
    let axis = "[[axis]]\nletter = \"X\"\nkind = \"linear\"\n";
    for key in ["0", "G54", "-1", "70000"] {
        let e = err(&minimal(&format!(
            "{axis}[work_offsets]\n\"{key}\" = {{ X = 1.0 }}"
        )));
        assert!(
            matches!(e, MachineDefError::InvalidWorkOffsetId(_)),
            "{key}"
        );
    }
    let e = err(&minimal(&format!(
        "{axis}[work_offsets]\n1 = {{ Q = 1.0 }}"
    )));
    assert!(matches!(
        e,
        MachineDefError::UnknownWorkOffsetAxis { id: 1, .. }
    ));
    let e = err(&minimal(&format!(
        "{axis}[work_offsets]\n1 = {{ X = nan }}"
    )));
    assert!(matches!(
        e,
        MachineDefError::InvalidWorkOffsetValue { id: 1, axis: 'X' }
    ));
}
