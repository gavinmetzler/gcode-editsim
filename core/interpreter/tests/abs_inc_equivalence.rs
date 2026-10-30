//! The same toolpath written in absolute (G90) and incremental (G91)
//! coordinates gives the same geometry (ARCHITECTURE.md section 11).

mod common;

use common::{moves, run};
use ir::bounding_box;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Step {
    Line(i32, i32, i32),
    /// Full circle around a centre offset (in thousandths of a mm).
    Circle(bool, i32),
    /// Arc by radius to a relative end point.
    Arc(bool, i32, i32),
}

fn step() -> impl Strategy<Value = Step> {
    let d = -20_000i32..20_000;
    prop_oneof![
        (d.clone(), d.clone(), -5_000i32..5_000).prop_map(|(x, y, z)| Step::Line(x, y, z)),
        (any::<bool>(), 1i32..30_000).prop_map(|(cw, r)| Step::Circle(cw, r)),
        (any::<bool>(), 1i32..10_000, 1i32..10_000).prop_map(|(cw, x, y)| Step::Arc(cw, x, y)),
    ]
}

/// Values are whole thousandths so both forms are exactly representable in
/// the program text.
fn mm(v: i32) -> String {
    format!("{:.3}", f64::from(v) / 1000.0)
}

fn programs(steps: &[Step]) -> (String, String) {
    let head = "G21 G17 G54 G90 G0 X0. Y0. Z0.\nG1 F500.\n";
    let mut abs = String::from(head);
    let mut inc = String::from(head);
    inc.push_str("G91\n");
    let (mut x, mut y, mut z) = (0i64, 0i64, 0i64);
    for s in steps {
        match *s {
            Step::Line(dx, dy, dz) => {
                x += i64::from(dx);
                y += i64::from(dy);
                z += i64::from(dz);
                abs.push_str(&format!("G1 X{} Y{} Z{}\n", mm3(x), mm3(y), mm3(z)));
                inc.push_str(&format!("G1 X{} Y{} Z{}\n", mm(dx), mm(dy), mm(dz)));
            }
            Step::Circle(cw, r) => {
                let g = if cw { "G2" } else { "G3" };
                abs.push_str(&format!("{g} X{} Y{} I{}\n", mm3(x), mm3(y), mm(r)));
                inc.push_str(&format!("{g} X0. Y0. I{}\n", mm(r)));
            }
            Step::Arc(cw, dx, dy) => {
                let g = if cw { "G2" } else { "G3" };
                // Radius comfortably longer than half the chord.
                let r = dx + dy;
                x += i64::from(dx);
                y += i64::from(dy);
                abs.push_str(&format!("{g} X{} Y{} R{}\n", mm3(x), mm3(y), mm(r)));
                inc.push_str(&format!("{g} X{} Y{} R{}\n", mm(dx), mm(dy), mm(r)));
            }
        }
    }
    (abs, inc)
}

fn mm3(v: i64) -> String {
    format!("{:.3}", v as f64 / 1000.0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn absolute_and_incremental_programs_match(steps in prop::collection::vec(step(), 1..60)) {
        let (abs, inc) = programs(&steps);
        let (pa, pi) = (run(&abs), run(&inc));
        let (ma, mi) = (moves(&pa), moves(&pi));
        prop_assert_eq!(ma.len(), mi.len(), "{:?}\n{:?}", pa.diagnostics, pi.diagnostics);
        for (a, i) in ma.iter().zip(&mi) {
            prop_assert_eq!(a.kind, i.kind);
            for axis in 0..3 {
                prop_assert!((a.end[axis] - i.end[axis]).abs() < 1e-6, "end {:?} vs {:?}", a.end, i.end);
            }
            // Same swept shape: compare each segment's exact bounding box,
            // which catches a full circle collapsing to a point.
            let (ba, bi) = (
                bounding_box(std::iter::once(*a)).unwrap(),
                bounding_box(std::iter::once(*i)).unwrap(),
            );
            for axis in 0..3 {
                prop_assert!((ba.min[axis] - bi.min[axis]).abs() < 1e-6, "{:?} vs {:?}", ba, bi);
                prop_assert!((ba.max[axis] - bi.max[axis]).abs() < 1e-6, "{:?} vs {:?}", ba, bi);
            }
        }
    }
}
