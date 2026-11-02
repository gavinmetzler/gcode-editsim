//! The path check for Osai 10 Series programs (M4.9a): translate, rotate
//! and scale random programs (I/J written as the centre's position, as the
//! 10 Series reads them in G90), run the original and the rewritten
//! program, and compare every move: the selection's moves (and arc
//! centres) are transformed, every other move is where it was.

mod common;

use common::{first_step_line, program_in, segments_with, steps, Style};
use dialect_osai::Osai;
use interpreter::Options;
use ir::{ArcPlane, Pos, MAX_AXES};
use machine_mill::Mill;
use proptest::prelude::*;
use transform::{apply_edits, transform, TransformError, Transformation};

/// Rounding to 0.001 mm per axis, plus a rounded start.
const TOLERANCE: f64 = 0.0011;
/// Arc centres are worked out from rounded values (see rotate_paths.rs).
const CENTRE_TOLERANCE: f64 = 0.005;

fn transformation() -> impl Strategy<Value = Transformation> {
    let centre = (-100_000i32..100_000, -100_000i32..100_000).prop_map(|(x, y)| {
        let mut c: Pos = [0.0; MAX_AXES];
        c[0] = f64::from(x) / 1000.0;
        c[1] = f64::from(y) / 1000.0;
        c
    });
    let xy = ArcPlane {
        first: 0,
        second: 1,
    };
    prop_oneof![
        (-50_000i32..50_000, -50_000i32..50_000, -5_000i32..5_000).prop_map(|(x, y, z)| {
            let mut offset: Pos = [0.0; MAX_AXES];
            offset[0] = f64::from(x) / 1000.0;
            offset[1] = f64::from(y) / 1000.0;
            offset[2] = f64::from(z) / 1000.0;
            Transformation::Translate { offset }
        }),
        (-360.0f64..360.0, centre.clone()).prop_map(move |(angle, centre)| {
            Transformation::Rotate {
                plane: xy,
                angle,
                centre,
            }
        }),
        (0.1f64..5.0, centre).prop_map(move |(factor, centre)| Transformation::Scale {
            plane: xy,
            factor,
            centre
        }),
    ]
}

/// Where a point in program coordinates goes.
fn expected(t: &Transformation, p: &Pos) -> Pos {
    let mut out = *p;
    match t {
        Transformation::Translate { offset } => {
            for (v, d) in out.iter_mut().zip(offset) {
                *v += d;
            }
        }
        Transformation::Rotate { angle, centre, .. } => {
            let (s, k) = angle.to_radians().sin_cos();
            let (x, y) = (p[0] - centre[0], p[1] - centre[1]);
            out[0] = centre[0] + x * k - y * s;
            out[1] = centre[1] + x * s + y * k;
        }
        Transformation::Scale { factor, centre, .. } => {
            out[0] = centre[0] + (p[0] - centre[0]) * factor;
            out[1] = centre[1] + (p[1] - centre[1]) * factor;
        }
    }
    out
}

fn within(a: &Pos, b: &Pos, axes: &[usize], tolerance: f64) -> bool {
    axes.iter().all(|&i| (a[i] - b[i]).abs() <= tolerance)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(600))]

    #[test]
    fn osai_selections_transform_and_nothing_else_moves(
        steps in steps(),
        a in 0usize..14,
        b in 0usize..14,
        t in transformation(),
    ) {
        let src = program_in(&steps, Style::Osai);
        let lines = u32::try_from(src.lines().count()).unwrap_or(0);
        let base = first_step_line(Style::Osai);
        let first = base + u32::try_from(a.min(b)).unwrap_or(0);
        let last = (base + u32::try_from(a.max(b)).unwrap_or(0)).min(lines);
        prop_assume!(first <= last);
        let osai = Osai::new().expect("dialect");
        let mill = Mill::generic().expect("machine");
        let result = match transform(src.as_bytes(), &osai, &mill, Options::default(), first..=last, &t) {
            Ok(r) => r,
            Err(TransformError::Refused { reason, .. }) => {
                prop_assert!(reason.contains("arc"), "unexpected refusal: {reason}\n{src}");
                return Ok(());
            }
        };
        let out = apply_edits(src.as_bytes(), &result.edits).expect("edits apply");
        let out_text = String::from_utf8_lossy(&out).into_owned();
        let before = segments_with(src.as_bytes(), &osai);
        let after = segments_with(&out, &osai);
        prop_assert_eq!(before.len(), after.len(), "{}\n--\n{}", src, out_text);
        // Origin 1 is work offset 1 of the generic mill: machine X-380 Y-205 Z-400.
        let origin = [-380.0, -205.0, -400.0];
        let to_work = |p: &Pos| { let mut q = *p; for i in 0..3 { q[i] -= origin[i]; } q };
        let to_machine = |p: Pos| { let mut q = p; for i in 0..3 { q[i] += origin[i]; } q };
        for (old, new) in before.iter().zip(&after) {
            prop_assert_eq!(old.src.line, new.src.line);
            let selected = (first..=last).contains(&old.src.line);
            let end = if selected { to_machine(expected(&t, &to_work(&old.end))) } else { old.end };
            prop_assert!(within(&new.end, &end, &[0, 1, 2], TOLERANCE), "line {}: {:?} != {:?}\n{}\n--\n{}", old.src.line, new.end, end, src, out_text);
            if let (Some(oa), Some(na)) = (&old.arc, &new.arc) {
                let c = if selected { to_machine(expected(&t, &to_work(&oa.center))) } else { oa.center };
                prop_assert!(within(&na.center, &c, &[0, 1], CENTRE_TOLERANCE), "arc centre on line {}\n{}\n--\n{}", old.src.line, src, out_text);
            }
        }
    }
}
