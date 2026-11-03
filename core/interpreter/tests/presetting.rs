//! M5.6e: presetting (Fanuc G92, Osai G92/G98) is drawn: the current
//! position becomes the programmed values through one shift of every work
//! coordinate system, until G92.1 (Fanuc) or G99 (Osai) cancels it.
//! Moves in machine coordinates do not see it.

use dialect_api::Dialect;
use dialect_fanuc::Fanuc;
use dialect_osai::Osai;
use interpreter::{interpret, Options};
use ir::{Program, SegKind};
use machine_api::MachineDef;
use machine_mill::Mill;

/// A 3-axis mill starting at machine zero, with G55 / origin 2 at X500 Y0.
fn machine() -> Mill {
    let text = "[machine]\nname = \"m\"\nfamily = \"mill\"\n\
        [[axis]]\nletter = \"X\"\nkind = \"linear\"\narc_offset = \"I\"\nreference = 0.0\n\
        [[axis]]\nletter = \"Y\"\nkind = \"linear\"\narc_offset = \"J\"\nreference = 0.0\n\
        [[axis]]\nletter = \"Z\"\nkind = \"linear\"\narc_offset = \"K\"\nreference = 0.0\n\
        [work_offsets]\n1 = { X = 0.0, Y = 0.0, Z = 0.0 }\n2 = { X = 500.0, Y = 0.0, Z = 0.0 }\n";
    Mill::new(MachineDef::from_toml_str(text).expect("valid")).expect("valid")
}

fn run(dialect: &dyn Dialect, src: &str) -> Program {
    interpret(
        src.as_bytes(),
        "t.nc",
        dialect,
        &machine(),
        Options::default(),
    )
}

/// The end points of the moves, X and Y in machine coordinates.
fn ends(p: &Program) -> Vec<(f64, f64)> {
    p.channels[0]
        .segments
        .iter()
        .filter(|s| matches!(s.kind, SegKind::Rapid | SegKind::Linear))
        .map(|s| (s.end[0], s.end[1]))
        .collect()
}

#[test]
fn the_osai_manuals_example() {
    // p. 2-82: G0 X100 Y80, then G92 X0 Y0 makes that point the origin.
    let osai = Osai::new().unwrap();
    let p = run(
        &osai,
        "G71 G17 G90\nG0 X100 Y80\nG92 X0 Y0\nG0 X10 Y10\nG99\nG0 X1 Y1\n",
    );
    assert_eq!(ends(&p), vec![(100.0, 80.0), (110.0, 90.0), (1.0, 1.0)]);
}

#[test]
fn fanuc_g92_shifts_every_work_coordinate_system_until_g92_1() {
    let fanuc = Fanuc::new().unwrap();
    let p = run(
        &fanuc,
        "G21 G90 G17 G54\nG0 X100. Y80.\nG92 X0 Y0\nG0 X10. Y10.\nG55 X0 Y0\nG92.1\nG0 X0 Y0\n",
    );
    // After G92 the tool at (100, 80) reads (0, 0) in G54 and every other
    // system: G55's origin moves by the same (100, 80).
    assert_eq!(
        ends(&p),
        vec![(100.0, 80.0), (110.0, 90.0), (600.0, 80.0), (500.0, 0.0)]
    );
}

#[test]
fn the_owners_router_start() {
    // `G54 G92 X1000. Y1250. Z0` with the tool at machine zero, then a
    // move to G55's zero: G55 is shifted by (-1000, -1250).
    let fanuc = Fanuc::new().unwrap();
    let p = run(
        &fanuc,
        "G21 G90 G17\nG54 G92 X1000. Y1250. Z0\nG90 G55 G00 X0 Y0 Z0\n",
    );
    assert_eq!(ends(&p), vec![(-500.0, -1250.0)]);
    assert!(
        p.diagnostics.iter().all(|d| !d.message.contains("G92")),
        "{:?}",
        p.diagnostics
    );
}

#[test]
fn an_axis_not_named_keeps_its_shift() {
    let fanuc = Fanuc::new().unwrap();
    let p = run(
        &fanuc,
        "G21 G90 G17 G54\nG0 X100. Y80.\nG92 X0 Y0\nG92 X50.\nG0 X0 Y0\n",
    );
    // X: the tool at 100 now reads 50, so X0 is machine 50; Y keeps its shift.
    assert_eq!(ends(&p).last(), Some(&(50.0, 80.0)));
}

#[test]
fn machine_coordinate_moves_do_not_see_the_preset() {
    let fanuc = Fanuc::new().unwrap();
    let p = run(
        &fanuc,
        "G21 G90 G17 G54\nG0 X100. Y80.\nG92 X0 Y0\nG53 G0 X5. Y5.\n",
    );
    let last = p.channels[0]
        .segments
        .iter()
        .rev()
        .find(|s| s.kind == SegKind::Rapid)
        .unwrap();
    assert_eq!((last.end[0], last.end[1]), (5.0, 5.0));
}
