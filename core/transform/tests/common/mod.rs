//! Random programs for the path checks (M4.6-M4.8): absolute moves,
//! incremental moves on random axes and XY arcs, all to whole thousandths.

#![allow(dead_code)]

use dialect_fanuc::Fanuc;
use interpreter::{interpret, Options};
use ir::{Pos, Segment};
use machine_mill::Mill;
use proptest::prelude::*;

/// One line of a generated program.
#[derive(Debug, Clone)]
pub enum Step {
    /// G90 move to X Y Z (thousandths of a mm).
    Absolute(bool, [i32; 3]),
    /// G91 move by the given axes (thousandths), others not written.
    Incremental([Option<i32>; 3]),
    /// G90 arc in XY about a centre offset from the start, by an angle.
    Arc(bool, [i32; 2], f64),
}

pub fn steps() -> impl Strategy<Value = Vec<Step>> {
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
pub fn program(steps: &[Step]) -> String {
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

pub fn segments(src: &[u8]) -> Vec<Segment> {
    let fanuc = Fanuc::new().expect("dialect");
    let mill = Mill::generic().expect("machine");
    let p = interpret(src, "t.nc", &fanuc, &mill, Options::default());
    p.channels
        .into_iter()
        .next()
        .map(|c| c.segments)
        .unwrap_or_default()
}

pub fn close(a: &Pos, b: &Pos) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
}

pub fn plus(p: &Pos, d: &Pos) -> Pos {
    let mut out = *p;
    for (v, o) in out.iter_mut().zip(d) {
        *v += o;
    }
    out
}
