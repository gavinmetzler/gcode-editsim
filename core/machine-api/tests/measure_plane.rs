//! M6 (owner, 2026-11-22): the measurement panel also gives the distance in
//! one plane of the drawing, XY for mills and routers. The machine file can
//! choose it (`measure_plane`); without it the machine type decides.

use machine_api::{MachineDef, MachineDefError, MeasurePlane};

fn load(machine_extra: &str) -> Result<MachineDef, MachineDefError> {
    MachineDef::from_toml_str(&format!(
        "[machine]\nname = \"m\"\nfamily = \"mill\"\n{machine_extra}\n\
         [[axis]]\nletter = \"X\"\nkind = \"linear\"\n\
         [[axis]]\nletter = \"Y\"\nkind = \"linear\"\n\
         [[axis]]\nletter = \"Z\"\nkind = \"linear\"\n"
    ))
}

#[test]
fn the_plane_is_optional() {
    assert_eq!(load("").expect("loads").measure_plane, None);
}

#[test]
fn the_plane_is_two_of_the_drawings_axes_in_any_letter_case() {
    let plane = |text: &str| {
        load(&format!("measure_plane = \"{text}\""))
            .expect("loads")
            .measure_plane
    };
    assert_eq!(plane("XY"), Some(MeasurePlane::XY));
    assert_eq!(plane("xz"), Some(MeasurePlane::XZ));
    assert_eq!(plane(" YZ "), Some(MeasurePlane::YZ));
}

#[test]
fn other_planes_are_refused() {
    for bad in ["", "X", "XX", "YX", "XA", "XYZ"] {
        let e = load(&format!("measure_plane = \"{bad}\"")).expect_err(bad);
        assert!(
            matches!(e, MachineDefError::InvalidMeasurePlane(_)),
            "{bad}: {e:?}"
        );
        assert!(e.to_string().contains("\"XY\""), "{e}");
    }
}

#[test]
fn each_plane_names_its_axes_of_the_drawing() {
    assert_eq!(MeasurePlane::XY.axes(), [0, 1]);
    assert_eq!(MeasurePlane::XZ.axes(), [0, 2]);
    assert_eq!(MeasurePlane::YZ.axes(), [1, 2]);
    assert_eq!(MeasurePlane::XZ.name(), "XZ");
}
