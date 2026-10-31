//! Canned cycles (G73, G74, G76, G81-G89) are not simulated (on hold,
//! owner decision 2026-10-31), but moves are never hidden: each hole is
//! drawn as a rapid to its position in the working plane. Depths, R
//! levels, pecks and retracts are not drawn. One warning per cycle code.
//! The cycle stays active for later blocks until G80 or G00-G03.

mod common;

use common::{run, w};
use dialect_api::Command;
use ir::{DiagCode, Severity};

#[test]
fn holes_are_rapids_in_the_plane() {
    let r = run("G17 G0 Z10.\nG81 X10. Y10. Z-5. R2. F100.\nX20.\nY30. Z-8.\nG80\nG0 X0\n");
    assert_eq!(
        r.moves(),
        vec![
            &Command::Rapid {
                target: vec![w(2, 10.0)]
            },
            &Command::Rapid {
                target: vec![w(0, 10.0), w(1, 10.0)]
            },
            &Command::Rapid {
                target: vec![w(0, 20.0)]
            },
            &Command::Rapid {
                target: vec![w(1, 30.0)]
            },
            &Command::Rapid {
                target: vec![w(0, 0.0)]
            },
        ]
    );
}

#[test]
fn one_warning_per_cycle_code() {
    let r =
        run("G17\nG81 X1. Y1. Z-5. R2. F100.\nX2.\nG80\nG81 X3. Z-5. R2.\nG83 X4. Z-9. R2. Q2.\n");
    let about: Vec<(u32, Severity)> = r
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::UnsupportedGCode)
        .map(|d| (d.src.line, d.severity))
        .collect();
    assert_eq!(about, vec![(2, Severity::Warning), (6, Severity::Warning)]);
    assert!(
        r.diags[0].message.contains("hole positions"),
        "{}",
        r.diags[0].message
    );
    assert!(!r
        .commands
        .iter()
        .any(|c| matches!(c, Command::PositionUnknown { .. })));
}

#[test]
fn g0_to_g3_cancel_the_cycle_like_g80() {
    let r = run("G17 G81 X10. Y10. Z-5. R2. F100.\nG0 Z50.\nX100.\n");
    assert_eq!(
        r.moves(),
        vec![
            &Command::Rapid {
                target: vec![w(0, 10.0), w(1, 10.0)]
            },
            &Command::Rapid {
                target: vec![w(2, 50.0)]
            },
            &Command::Rapid {
                target: vec![w(0, 100.0)]
            },
        ]
    );
    for code in ["G1 X5. F100.", "G2 X5. R5. F100.", "G3 X5. R5. F100."] {
        let r = run(&format!("G17 G81 X1. Y1. Z-5. R2. F100.\n{code}\n"));
        assert_eq!(r.moves().len(), 2, "{code}: {:?}", r.diags);
    }
}
