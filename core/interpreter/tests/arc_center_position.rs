//! `ArcCenter::Position`: an arc centre programmed like an end point
//! (absolute in work coordinates, or incremental from the start), as some
//! controls (e.g. Osai 10 Series in G90) program I and J. Uses a scripted
//! dialect so only the interpreter is under test.

use std::cell::RefCell;

use dialect_api::{
    ArcCenter, ArcDir, AxisSet, AxisWord, BlockContext, CodeTable, Command, Dialect, ModalState,
};
use gcode_syntax::Block;
use interpreter::{interpret, Options};
use ir::{ArcPlane, Diagnostics, Program, SegKind, Segment};
use machine_lathe::Lathe;
use machine_mill::Mill;

/// Emits the scripted commands for each block, in order.
struct Scripted {
    blocks: RefCell<Vec<Vec<Command>>>,
    table: CodeTable,
}

impl Scripted {
    fn new(blocks: Vec<Vec<Command>>) -> Self {
        Self {
            blocks: RefCell::new(blocks.into_iter().rev().collect()),
            table: CodeTable::from_toml_str("[dialect]\nid = \"scripted\"\n").expect("table"),
        }
    }
}

impl Dialect for Scripted {
    fn id(&self) -> &str {
        "scripted"
    }
    fn code_table(&self) -> &CodeTable {
        &self.table
    }
    fn initial_state(&self, _axes: &AxisSet) -> ModalState {
        ModalState::default()
    }
    fn interpret_block(
        &self,
        _block: &Block,
        _ctx: &BlockContext<'_>,
        _state: &mut ModalState,
        out: &mut Vec<Command>,
        _diag: &mut Diagnostics,
    ) {
        if let Some(cmds) = self.blocks.borrow_mut().pop() {
            out.extend(cmds);
        }
    }
}

const XY: ArcPlane = ArcPlane {
    first: 0,
    second: 1,
};

fn abs(axis: usize, value: f64) -> AxisWord {
    AxisWord {
        axis,
        value,
        incremental: false,
    }
}

fn inc(axis: usize, value: f64) -> AxisWord {
    AxisWord {
        axis,
        value,
        incremental: true,
    }
}

fn run_on(machine: &dyn machine_api::Machine, blocks: Vec<Vec<Command>>) -> Program {
    let source = "x\n".repeat(blocks.len());
    interpret(
        source.as_bytes(),
        "t.nc",
        &Scripted::new(blocks),
        machine,
        Options::default(),
    )
}

fn arcs(p: &Program) -> Vec<&Segment> {
    p.channels[0]
        .segments
        .iter()
        .filter(|s| s.kind.is_arc())
        .collect()
}

fn mill_blocks(center: ArcCenter, target: Vec<AxisWord>) -> Vec<Vec<Command>> {
    vec![
        vec![Command::SetWorkOffset(ir::WorkOffsetId(1))],
        // Start at work X10 Y20, i.e. machine X-370 Y-185 (G54 is
        // X-380 Y-205 on the generic mill).
        vec![
            Command::SetFeedRate(100.0),
            Command::Rapid {
                target: vec![abs(0, 10.0), abs(1, 20.0)],
            },
        ],
        vec![Command::Arc {
            target,
            center,
            dir: ArcDir::Cw,
            plane: XY,
        }],
    ]
}

#[test]
fn absolute_centre_is_in_work_coordinates() {
    // The manual's example: from X10 Y20, G2 X46 Y20 I28 J20 (centre at
    // work X28 Y20, radius 18).
    let mill = Mill::generic().expect("mill");
    let p = run_on(
        &mill,
        mill_blocks(
            ArcCenter::Position(vec![abs(0, 28.0), abs(1, 20.0)]),
            vec![abs(0, 46.0), abs(1, 20.0)],
        ),
    );
    let a = arcs(&p);
    assert_eq!(a.len(), 1, "{:?}", p.diagnostics);
    let geom = a[0].arc.expect("geometry");
    assert!((geom.center[0] - (-380.0 + 28.0)).abs() < 1e-9);
    assert!((geom.center[1] - (-205.0 + 20.0)).abs() < 1e-9);
    assert!((geom.radius - 18.0).abs() < 1e-9);
    assert_eq!(a[0].kind, SegKind::ArcCw);
}

#[test]
fn incremental_centre_is_from_the_start() {
    let mill = Mill::generic().expect("mill");
    let p = run_on(
        &mill,
        mill_blocks(
            ArcCenter::Position(vec![inc(0, 18.0), inc(1, 0.0)]),
            vec![inc(0, 36.0)],
        ),
    );
    let geom = arcs(&p)[0].arc.expect("geometry");
    assert!((geom.center[0] - (-370.0 + 18.0)).abs() < 1e-9);
    assert!((geom.center[1] - (-185.0)).abs() < 1e-9);
}

#[test]
fn unlisted_plane_axis_takes_the_start_coordinate() {
    let mill = Mill::generic().expect("mill");
    let p = run_on(
        &mill,
        mill_blocks(
            ArcCenter::Position(vec![abs(0, 28.0)]),
            vec![abs(0, 46.0), abs(1, 20.0)],
        ),
    );
    let geom = arcs(&p)[0].arc.expect("geometry");
    assert!((geom.center[1] - (-185.0)).abs() < 1e-9);
}

#[test]
fn no_end_point_is_a_full_circle() {
    let mill = Mill::generic().expect("mill");
    let p = run_on(
        &mill,
        mill_blocks(
            ArcCenter::Position(vec![abs(0, 28.0), abs(1, 20.0)]),
            vec![],
        ),
    );
    let a = arcs(&p);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].start, a[0].end);
    assert!((a[0].arc.expect("geometry").radius - 18.0).abs() < 1e-9);
}

#[test]
fn centre_on_a_diameter_axis_is_converted_like_an_end_point() {
    // Lathe: X is a diameter axis, G54 at X0 Z100. From X40 (r 20) Z0 to
    // X50 (r 25) Z-5 with the centre at diameter X40 (r 20), Z-5.
    let lathe = Lathe::generic().expect("lathe");
    let zx = ArcPlane {
        first: 1,
        second: 0,
    };
    let p = run_on(
        &lathe,
        vec![
            vec![Command::SetWorkOffset(ir::WorkOffsetId(1))],
            vec![
                Command::SetFeedRate(100.0),
                Command::Rapid {
                    target: vec![abs(0, 40.0), abs(1, 0.0)],
                },
            ],
            vec![Command::Arc {
                target: vec![abs(0, 50.0), abs(1, -5.0)],
                center: ArcCenter::Position(vec![abs(1, -5.0), abs(0, 40.0)]),
                dir: ArcDir::Ccw,
                plane: zx,
            }],
        ],
    );
    let a = arcs(&p);
    assert_eq!(a.len(), 1, "{:?}", p.diagnostics);
    let geom = a[0].arc.expect("geometry");
    assert!((geom.center[0] - 20.0).abs() < 1e-9);
    assert!((geom.center[1] - 95.0).abs() < 1e-9);
    assert!((geom.radius - 5.0).abs() < 1e-9);
}

#[test]
fn mismatched_radius_is_still_rejected() {
    let mill = Mill::generic().expect("mill");
    let p = run_on(
        &mill,
        mill_blocks(
            // End radius sqrt(18² + 1²) = 18.028, 0.028 more than the start's.
            ArcCenter::Position(vec![abs(0, 28.0), abs(1, 20.0)]),
            vec![abs(0, 46.0), abs(1, 21.0)],
        ),
    );
    assert!(arcs(&p).is_empty());
    assert!(p
        .diagnostics
        .iter()
        .any(|d| d.code == ir::DiagCode::ArcRadiusMismatch));
}
