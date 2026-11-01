//! The check for Rotate (M4.7, M4_PLAN §4): run the original and the
//! rewritten program; the selection's moves are the original ones turned
//! about the centre (arc centres too), and every other move is where it
//! was. Turned values are rounded to the least increment (0.001 mm), so
//! positions agree within about one increment.

mod common;

use common::{program, segments, steps};
use dialect_fanuc::Fanuc;
use interpreter::Options;
use ir::{ArcPlane, Pos, MAX_AXES};
use machine_mill::Mill;
use proptest::prelude::*;
use transform::{apply_edits, transform, TransformError, Transformation};

/// Rounding to 0.001 mm on each axis, plus the start of a move that was
/// itself rounded.
const TOLERANCE: f64 = 0.0011;

fn turned(p: &Pos, angle: f64, c: (f64, f64)) -> Pos {
    let (s, k) = angle.to_radians().sin_cos();
    let (x, y) = (p[0] - c.0, p[1] - c.1);
    let mut out = *p;
    out[0] = c.0 + x * k - y * s;
    out[1] = c.1 + x * s + y * k;
    out
}

fn near(a: &Pos, b: &Pos, axes: &[usize]) -> bool {
    axes.iter().all(|&i| (a[i] - b[i]).abs() <= TOLERANCE)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(800))]

    #[test]
    fn the_selection_turns_about_the_centre_and_nothing_else_moves(
        steps in steps(),
        a in 0usize..14,
        b in 0usize..14,
        angle in -360.0f64..360.0,
        cx in -100_000i32..100_000,
        cy in -100_000i32..100_000,
    ) {
        let src = program(&steps);
        let lines = u32::try_from(src.lines().count()).unwrap_or(0);
        let first = 3 + u32::try_from(a.min(b)).unwrap_or(0);
        let last = (3 + u32::try_from(a.max(b)).unwrap_or(0)).min(lines);
        prop_assume!(first <= last);
        let c = (f64::from(cx) / 1000.0, f64::from(cy) / 1000.0);
        let mut centre: Pos = [0.0; MAX_AXES];
        centre[0] = c.0;
        centre[1] = c.1;
        let t = Transformation::Rotate { plane: ArcPlane { first: 0, second: 1 }, angle, centre };
        let fanuc = Fanuc::new().expect("dialect");
        let mill = Mill::generic().expect("machine");
        let result = match transform(src.as_bytes(), &fanuc, &mill, Options::default(), first..=last, &t) {
            Ok(r) => r,
            Err(TransformError::Refused { reason, .. }) => {
                prop_assert!(reason.contains("arc"), "unexpected refusal: {reason}\n{src}");
                return Ok(());
            }
        };
        let out = apply_edits(src.as_bytes(), &result.edits).expect("edits apply");
        let out_text = String::from_utf8_lossy(&out).into_owned();
        let before = segments(src.as_bytes());
        let after = segments(&out);
        prop_assert_eq!(before.len(), after.len(), "{}\n--\n{}", src, out_text);
        // The work offset (G54 at machine X-380 Y-205): turn in program
        // coordinates.
        let origin = (-380.0, -205.0);
        let to_machine = |p: Pos| { let mut q = p; q[0] += origin.0; q[1] += origin.1; q };
        let to_work = |p: &Pos| { let mut q = *p; q[0] -= origin.0; q[1] -= origin.1; q };
        for (old, new) in before.iter().zip(&after) {
            prop_assert_eq!(old.src.line, new.src.line);
            let selected = (first..=last).contains(&old.src.line);
            let expected = if selected { to_machine(turned(&to_work(&old.end), angle, c)) } else { old.end };
            prop_assert!(near(&new.end, &expected, &[0, 1, 2]), "line {}: {:?} != {:?}\n{}\n--\n{}", old.src.line, new.end, expected, src, out_text);
            if let (Some(oa), Some(na)) = (&old.arc, &new.arc) {
                let centre = if selected { to_machine(turned(&to_work(&oa.center), angle, c)) } else { oa.center };
                prop_assert!(near(&na.center, &centre, &[0, 1]), "arc centre on line {}\n{}\n--\n{}", old.src.line, src, out_text);
                prop_assert_eq!(oa.plane, na.plane);
            }
        }
    }
}
