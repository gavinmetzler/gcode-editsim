//! The check for Translate (M4.6, M4_PLAN §4): run the original and the
//! rewritten program; the selection's moves are the original ones moved by
//! the offset, and every other move is where it was.

mod common;

use common::{close, plus, program, segments, steps};
use dialect_fanuc::Fanuc;
use interpreter::Options;
use ir::{Pos, MAX_AXES};
use machine_mill::Mill;
use proptest::prelude::*;
use transform::{apply_edits, transform, TransformError, Transformation};

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
