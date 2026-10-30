//! The Osai 10 Series dialect stub through the whole pipeline, on the
//! generic mill (origin 1 = machine X-380 Y-205 Z-400).

use dialect_osai::Osai;
use interpreter::{interpret, Options};
use ir::{Program, SegKind, Segment, Severity};
use machine_mill::Mill;

fn run(src: &str) -> Program {
    let osai = Osai::new().expect("dialect loads");
    let mill = Mill::generic().expect("machine loads");
    interpret(src.as_bytes(), "test.nc", &osai, &mill, Options::default())
}

fn segments(p: &Program) -> &[Segment] {
    &p.channels[0].segments
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn manual_arc_example_in_origin_1() {
    // Manual p. 2-4: N14 X10 Y20 / N15 G2 X46 Y20 I28 J20 F200 /
    // N16 G3 X64 Y38 I46 J38.
    let p = run(
        "; arc example\nG71\nG17\n(UAO,1)\nN14 X10 Y20 Z5 M3 S1000\nN15 G2 X46 Y20 I28 J20 F200\n\
         N16 G3 X64 Y38 I46 J38\nM30\n",
    );
    let errors: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.severity != Severity::Info)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    let kinds: Vec<SegKind> = segments(&p).iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        vec![
            SegKind::SpindleCw,
            SegKind::Rapid,
            SegKind::ArcCw,
            SegKind::ArcCcw,
            SegKind::ProgramEnd
        ]
    );
    let segs = segments(&p);
    // Starts at home (machine zero), not at an origin.
    assert_eq!(segs[1].start[..3], [0.0, 0.0, 0.0]);
    assert!(close(segs[1].end[0], -370.0) && close(segs[1].end[1], -185.0));
    let a = segs[2].arc.expect("arc");
    assert!(close(a.center[0], -352.0) && close(a.center[1], -185.0));
    assert!(close(a.radius, 18.0));
    let b = segs[3].arc.expect("arc");
    assert!(close(b.center[0], -334.0) && close(b.center[1], -167.0));
    assert!(close(segs[3].end[0], -316.0) && close(segs[3].end[1], -167.0));
}

#[test]
fn incremental_form_gives_the_same_path() {
    let abs = run("G71 (UAO,1)\nX10 Y20 Z5\nG2 X46 Y20 I28 J20 F200\nG3 X64 Y38 I46 J38\n");
    let inc = run("G71 (UAO,1)\nX10 Y20 Z5\nG2 G91 X36 I18 J0 F200\nG3 X18 Y18 I0 J18\n");
    let ends = |p: &Program| -> Vec<(f64, f64)> {
        segments(p).iter().map(|s| (s.end[0], s.end[1])).collect()
    };
    let (a, b) = (ends(&abs), ends(&inc));
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert!(close(x.0, y.0) && close(x.1, y.1), "{a:?} vs {b:?}");
    }
}

#[test]
fn full_circle_from_centre_only() {
    let p = run("G71 (UAO,1)\nX10 Y20 Z5\nG2 I28 J20 F200\n");
    let arc = segments(&p).iter().find(|s| s.kind.is_arc()).expect("arc");
    assert_eq!(arc.start, arc.end);
    assert!(close(arc.arc.expect("geometry").radius, 18.0));
}

#[test]
fn an_unreadable_line_stops_drawing_until_the_position_is_known() {
    let p = run("G71 X10 Y10 Z10\n\"START\" N125 X0\nX20\nX30 Y30 Z30\nX40\n");
    let rapids: Vec<f64> = segments(&p)
        .iter()
        .filter(|s| s.kind == SegKind::Rapid)
        .map(|s| s.end[0])
        .collect();
    // X20 is not drawn (Y and Z unknown); X30 Y30 Z30 makes every axis known.
    assert_eq!(rapids, vec![10.0, 40.0]);
}
