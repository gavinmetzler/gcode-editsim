//! The interpreter on a second machine type: the 2-axis lathe stub with
//! diameter programming on X. This checks the machine traits, not lathe
//! G-code: it uses the Fanuc milling dialect, whose G codes for these
//! basic moves match (a Fanuc lathe dialect is not in scope).

use dialect_fanuc::Fanuc;
use interpreter::{interpret, Options};
use ir::{DiagCode, Program, SegKind, Segment, Severity};
use machine_api::Machine;
use machine_lathe::Lathe;

/// Runs `src` on the shipped lathe: X is a diameter, machine coordinates
/// are radius; G54 is at X0 Z100; the reference position is X150 Z200.
fn run(src: &str) -> Program {
    let fanuc = Fanuc::new().expect("dialect loads");
    let lathe = Lathe::generic().expect("machine loads");
    interpret(
        src.as_bytes(),
        "test.nc",
        &fanuc,
        &lathe,
        Options::default(),
    )
}

fn moves(p: &Program) -> Vec<&Segment> {
    p.channels[0]
        .segments
        .iter()
        .filter(|s| s.kind.is_motion())
        .collect()
}

fn errors(p: &Program) -> Vec<(DiagCode, String)> {
    p.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| (d.code, d.message.clone()))
        .collect()
}

/// X (radius) and Z of a position.
fn xz(p: &ir::Pos) -> (f64, f64) {
    (p[0], p[1])
}

#[test]
fn starts_at_the_reference_position() {
    let p = run("G21 G90 G54 G0 Z5.\n");
    assert_eq!(xz(&moves(&p)[0].start), (150.0, 200.0));
}

#[test]
fn diameter_values_become_radius_positions() {
    let p = run("G21 G90 G54 G0 X50. Z5.\nG1 X40. F100.\n");
    assert!(errors(&p).is_empty(), "{:?}", errors(&p));
    let m = moves(&p);
    // X50 diameter = 25 radius; Z5 from a part zero at Z100 is 105.
    assert_eq!(xz(&m[0].end), (25.0, 105.0));
    assert_eq!(xz(&m[1].end), (20.0, 105.0));
}

#[test]
fn incremental_diameter_moves_are_halved_too() {
    let p = run("G21 G90 G54 G0 X50. Z5.\nG91 G1 X-10. Z-2. F100.\n");
    let m = moves(&p);
    assert_eq!(xz(&m[1].end), (20.0, 103.0));
}

#[test]
fn arcs_in_the_zx_plane() {
    // A 90° corner of radius 5: from X40 (radius 20) Z0 to X50 (radius 25)
    // Z-5, centre at radius 20, Z-5. Seen from the centre, the start lies
    // toward +Z and the end toward +X. In the ZX plane (Z first, X second)
    // turning from +Z to +X is counter-clockwise, so G3.
    let p = run("G21 G90 G54 G18 G0 X40. Z0.\nG3 X50. Z-5. R5. F100.\n");
    assert!(errors(&p).is_empty(), "{:?}", errors(&p));
    let arc = moves(&p)[1];
    assert_eq!(arc.kind, SegKind::ArcCcw);
    assert_eq!(xz(&arc.end), (25.0, 95.0));
    let geom = arc.arc.expect("arc geometry");
    assert!((geom.radius - 5.0).abs() < 1e-9);
    assert!((geom.center[0] - 20.0).abs() < 1e-9);
    assert!((geom.center[1] - 95.0).abs() < 1e-9);
}

#[test]
fn arc_without_a_plane_is_reported_not_drawn() {
    // The Fanuc dialect assumes G17 (XY) when no plane is programmed; a
    // lathe has no Y axis, so there is no plane and the arc is an error.
    // (Finding for the dialect traits: the assumed power-on plane depends
    // on the machine. See ARCHITECTURE.md §12.)
    let p = run("G21 G90 G54 G0 X40. Z0.\nG3 X50. Z-5. R5. F100.\n");
    assert_eq!(moves(&p).len(), 1);
    assert!(!errors(&p).is_empty());
}

#[test]
fn y_words_are_not_axes_on_a_lathe() {
    let p = run("G21 G90 G54 G0 X40. Y3. Z0.\n");
    assert!(p
        .diagnostics
        .iter()
        .any(|d| d.code == DiagCode::UnsupportedWord && d.message.contains('Y')));
}

#[test]
fn travel_limits_are_checked_in_radius() {
    // X300 diameter = 150 radius: at the limit, not over it.
    let ok = run("G21 G90 G54 G0 X300. Z5.\n");
    assert!(ok
        .diagnostics
        .iter()
        .all(|d| d.code != DiagCode::OutOfLimits));
    let over = run("G21 G90 G54 G0 X302. Z5.\n");
    assert!(over
        .diagnostics
        .iter()
        .any(|d| d.code == DiagCode::OutOfLimits));
}

#[test]
fn reference_return_goes_to_the_radius_reference() {
    let p = run("G21 G90 G54 G0 X50. Z5.\nG28 X50. Z5.\n");
    let last = moves(&p).last().copied().expect("moves");
    assert_eq!(xz(&last.end), (150.0, 200.0));
}

#[test]
fn render_kinematics_puts_the_profile_in_the_xz_plane() {
    let lathe = Lathe::generic().expect("machine loads");
    let p = run("G21 G90 G54 G0 X50. Z5.\n");
    let end = moves(&p)[0].end;
    assert_eq!(lathe.forward_kinematics(&end).position, [25.0, 0.0, 105.0]);
}
