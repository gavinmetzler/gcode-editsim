//! The check for Translate (M4.6, M4_PLAN §4): run the original and the
//! rewritten program; the selection's moves are the original ones moved by
//! the offset, and every other move is where it was.

use dialect_fanuc::Fanuc;
use interpreter::{interpret, Options};
use ir::{Pos, Segment, MAX_AXES};
use machine_mill::Mill;
use proptest::prelude::*;
use transform::{apply_edits, transform, TransformError, Transformation};

/// One line of a generated program.
#[derive(Debug, Clone)]
enum Step {
    /// G90 move to X Y Z (thousandths of a mm).
    Absolute(bool, [i32; 3]),
    /// G91 move by the given axes (thousandths), others not written.
    Incremental([Option<i32>; 3]),
    /// G90 arc in XY about a centre offset from the start, by an angle.
    Arc(bool, [i32; 2], f64),
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    let value = -100_000i32..100_000;
    let delta = proptest::option::of(-20_000i32..20_000);
    let step = prop_oneof![
        3 => (any::<bool>(), [value.clone(), value.clone(), value]).prop_map(|(r, p)| Step::Absolute(r, p)),
        3 => [delta.clone(), delta.clone(), delta].prop_map(Step::Incremental),
        2 => (any::<bool>(), [1_000i32..30_000, -30_000i32..30_000], 0.2f64..5.0)
            .prop_map(|(cw, c, a)| Step::Arc(cw, c, a)),
    ];
    proptest::collection::vec(step, 1..14)
}

fn mm(thousandths: i32) -> String {
    format!("{:.3}", f64::from(thousandths) / 1000.0)
}

/// The program text for `steps`, after a header on lines 1 and 2.
fn program(steps: &[Step]) -> String {
    let mut text = String::from("G21 G90 G17 G54 G0 X0. Y0. Z5.\nG1 F100.\n");
    let mut at = [0.0f64, 0.0, 5.0];
    for step in steps {
        match step {
            Step::Absolute(rapid, p) => {
                let code = if *rapid { "G0" } else { "G1" };
                text.push_str(&format!(
                    "G90 {code} X{} Y{} Z{}\n",
                    mm(p[0]),
                    mm(p[1]),
                    mm(p[2])
                ));
                at = [p[0], p[1], p[2]].map(|v| f64::from(v) / 1000.0);
            }
            Step::Incremental(d) => {
                if d.iter().all(Option::is_none) {
                    continue;
                }
                let mut line = String::from("G91 G1");
                for (i, (letter, v)) in ['X', 'Y', 'Z'].iter().zip(d).enumerate() {
                    if let Some(v) = v {
                        line.push_str(&format!(" {letter}{}", mm(*v)));
                        at[i] += f64::from(*v) / 1000.0;
                    }
                }
                text.push_str(&line);
                text.push('\n');
                // Keep positions to whole thousandths, as written.
                at = at.map(|v| (v * 1000.0).round() / 1000.0);
            }
            Step::Arc(cw, c, angle) => {
                let (i, j) = (f64::from(c[0]) / 1000.0, f64::from(c[1]) / 1000.0);
                let (cx, cy) = (at[0] + i, at[1] + j);
                let (sx, sy) = (at[0] - cx, at[1] - cy);
                let turn = if *cw { -angle } else { *angle };
                let (ex, ey) = (
                    cx + sx * turn.cos() - sy * turn.sin(),
                    cy + sx * turn.sin() + sy * turn.cos(),
                );
                let (ex, ey) = ((ex * 1000.0).round() as i32, (ey * 1000.0).round() as i32);
                let code = if *cw { "G2" } else { "G3" };
                text.push_str(&format!(
                    "G90 {code} X{} Y{} I{} J{}\n",
                    mm(ex),
                    mm(ey),
                    mm(c[0]),
                    mm(c[1])
                ));
                at[0] = f64::from(ex) / 1000.0;
                at[1] = f64::from(ey) / 1000.0;
            }
        }
    }
    text
}

fn segments(src: &[u8]) -> Vec<Segment> {
    let fanuc = Fanuc::new().expect("dialect");
    let mill = Mill::generic().expect("machine");
    let p = interpret(src, "t.nc", &fanuc, &mill, Options::default());
    p.channels
        .into_iter()
        .next()
        .map(|c| c.segments)
        .unwrap_or_default()
}

fn close(a: &Pos, b: &Pos) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
}

fn plus(p: &Pos, d: &Pos) -> Pos {
    let mut out = *p;
    for (v, o) in out.iter_mut().zip(d) {
        *v += o;
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn the_selection_moves_by_the_offset_and_nothing_else_moves(
        steps in steps(),
        a in 0usize..14,
        b in 0usize..14,
        d in [-50_000i32..50_000, -50_000i32..50_000, -5_000i32..5_000],
    ) {
        let src = program(&steps);
        let lines = u32::try_from(src.lines().count()).unwrap_or(0);
        let first = 3 + u32::try_from(a.min(b)).unwrap_or(0);
        let last = (3 + u32::try_from(a.max(b)).unwrap_or(0)).min(lines);
        prop_assume!(first <= last);
        let mut offset: Pos = [0.0; MAX_AXES];
        for (o, v) in offset.iter_mut().zip(d) {
            *o = f64::from(v) / 1000.0;
        }
        let fanuc = Fanuc::new().expect("dialect");
        let mill = Mill::generic().expect("machine");
        let result = transform(
            src.as_bytes(), &fanuc, &mill, Options::default(), first..=last,
            &Transformation::Translate { offset },
        );
        let result = match result {
            Ok(r) => r,
            // An arc can neither start nor follow the selection.
            Err(TransformError::Refused { reason, .. }) => {
                prop_assert!(reason.contains("arc"), "unexpected refusal: {reason}\n{src}");
                return Ok(());
            }
        };
        let out = apply_edits(src.as_bytes(), &result.edits).expect("edits apply");
        let before = segments(src.as_bytes());
        let after = segments(&out);
        let out_text = String::from_utf8_lossy(&out).into_owned();
        prop_assert_eq!(before.len(), after.len(), "{}\n--\n{}", src, out_text);
        for (old, new) in before.iter().zip(&after) {
            prop_assert_eq!(old.src.line, new.src.line);
            let selected = (first..=last).contains(&old.src.line);
            let expected = if selected { plus(&old.end, &offset) } else { old.end };
            prop_assert!(close(&new.end, &expected), "line {}: {:?} != {:?}\n{}\n--\n{}", old.src.line, new.end, expected, src, out_text);
            if let (Some(oa), Some(na)) = (&old.arc, &new.arc) {
                // Only the two plane axes of a centre mean anything.
                let centre = if selected { plus(&oa.center, &offset) } else { oa.center };
                for axis in [usize::from(na.plane.first), usize::from(na.plane.second)] {
                    prop_assert!((na.center[axis] - centre[axis]).abs() < 1e-6, "arc centre on line {}", old.src.line);
                }
            }
        }
    }
}
