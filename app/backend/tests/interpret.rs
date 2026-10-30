//! Machine-type lookup, dialect choice and interpretation to a render
//! buffer.

use app_backend::{
    dialects, interpret_to_buffer, load_dialect, load_machine, BackendError, RunOptions,
};

const MILL: &str = machine_mill::EXAMPLE_DEFINITION;
const LATHE: &str = machine_lathe::EXAMPLE_DEFINITION;

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn f64_at(b: &[u8], at: usize) -> f64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[at..at + 8]);
    f64::from_le_bytes(a)
}

#[test]
fn family_picks_the_machine_type() {
    assert_eq!(
        load_machine(MILL).expect("mill").name(),
        "Generic 3-axis VMC"
    );
    assert_eq!(load_machine(LATHE).expect("lathe").axes().len(), 2);
}

#[test]
fn unknown_family_is_an_error() {
    let text = MILL.replace("family = \"mill\"", "family = \"router\"");
    assert!(matches!(
        load_machine(&text),
        Err(BackendError::UnknownFamily(f)) if f == "router"
    ));
}

#[test]
fn bad_machine_files_are_errors() {
    assert!(matches!(
        load_machine("not toml ["),
        Err(BackendError::Machine(_))
    ));
    // Valid TOML, but a lathe file claiming to be a mill.
    let text = LATHE.replace("family = \"lathe\"", "family = \"mill\"");
    let err = load_machine(&text).err().expect("rejected");
    assert!(err.to_string().contains("3-axis mill"), "{err}");
}

#[test]
fn both_dialects_are_offered_and_load() {
    let ids: Vec<&str> = dialects().iter().map(|d| d.id).collect();
    assert_eq!(ids, vec!["fanuc", "osai"]);
    assert!(dialects()[1].stub && !dialects()[0].stub);
    for d in dialects() {
        assert_eq!(load_dialect(d.id).expect("loads").id(), d.id);
    }
    assert!(matches!(
        load_dialect("haas"),
        Err(BackendError::UnknownDialect(_))
    ));
}

#[test]
fn buffer_holds_the_program() {
    let program = "G21 G90 G54 G0 X0. Y0. Z5.\nG1 Z-1. F100.\nG2 X10. Y0. R5.\nM30\n";
    let b = interpret_to_buffer(
        program.as_bytes(),
        "t.nc",
        MILL,
        "fanuc",
        RunOptions::default(),
    )
    .expect("runs");
    assert_eq!(u32_at(&b, 0), ir::render::layout::MAGIC);
    assert_eq!(u32_at(&b, 4) & 0xFFFF, 1, "version 1");
    // Rapid, linear, arc, program end.
    assert_eq!(u32_at(&b, 12), 4);
}

#[test]
fn problems_in_the_program_are_diagnostics_not_errors() {
    let b = interpret_to_buffer(b"G999\nX\n", "t.nc", MILL, "fanuc", RunOptions::default())
        .expect("runs");
    assert!(u32_at(&b, 16) >= 2, "diagnostics in the buffer");
}

#[test]
fn the_machine_decides_where_points_are_drawn() {
    // Lathe: X50 (diameter) is radius 25, drawn at x = 25, y = 0; Z5 from
    // the part zero at Z100 is z = 105.
    let b = interpret_to_buffer(
        b"G21 G90 G54 G0 X50. Z5.\n",
        "t.nc",
        LATHE,
        "fanuc",
        RunOptions::default(),
    )
    .expect("runs");
    let origin = [f64_at(&b, 24), f64_at(&b, 32), f64_at(&b, 40)];
    let vertices = u32_at(&b, 48) as usize;
    let last = vertices + 12; // second vertex: the rapid's end
    let p = [
        origin[0] + f64::from(f32_at(&b, last)),
        origin[1] + f64::from(f32_at(&b, last + 4)),
        origin[2] + f64::from(f32_at(&b, last + 8)),
    ];
    assert!(
        (p[0] - 25.0).abs() < 1e-6 && p[1] == 0.0 && (p[2] - 105.0).abs() < 1e-6,
        "{p:?}"
    );
}

#[test]
fn osai_runs_too() {
    let b = interpret_to_buffer(
        b"G71\n(UAO,1)\nX10 Y20\n",
        "t.nc",
        MILL,
        "osai",
        RunOptions::default(),
    )
    .expect("runs");
    assert_eq!(u32_at(&b, 12), 1, "one rapid");
}

#[test]
fn tolerance_must_be_positive() {
    for tolerance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let options = RunOptions {
            tolerance,
            ..RunOptions::default()
        };
        assert!(matches!(
            interpret_to_buffer(b"", "t.nc", MILL, "fanuc", options),
            Err(BackendError::InvalidTolerance)
        ));
    }
}

#[test]
fn block_delete_is_passed_on() {
    let program = b"G21 G90 G54\n/G0 X1. Y1. Z1.\n";
    let segments = |block_delete| {
        let options = RunOptions {
            block_delete,
            ..RunOptions::default()
        };
        let b = interpret_to_buffer(program, "t.nc", MILL, "fanuc", options).expect("runs");
        u32_at(&b, 12)
    };
    assert_eq!((segments(false), segments(true)), (1, 0));
}
