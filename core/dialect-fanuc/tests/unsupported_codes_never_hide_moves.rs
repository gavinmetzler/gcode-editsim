//! Codes that change how later blocks move but are not simulated (shifts,
//! rotation, scaling, mirroring, polar input, offset setting, macro and
//! subprogram calls, unknown G codes) never hide moves (owner decision,
//! 2026-10-31): a backplot drawn without their effect is more useful than
//! none. Each is reported once where it appears; its words are data, never
//! a move; later moves are drawn as programmed.

mod common;

use common::{run, w};
use dialect_api::Command;
use ir::{DiagCode, Severity};

const CODES: [&str; 10] = [
    "G92 X100. Y100.",
    "G52 X10. Y10.",
    "G68 X0 Y0 R90.",
    "G51 X0 Y0 P2",
    "G51.1 X0.",
    "G16",
    "G10 L2 P1 X-100.",
    "G65 P9000 A1.",
    "M98 P1000",
    "G33 Z-10. F1.5",
];

#[test]
fn later_moves_are_drawn() {
    for code in CODES {
        let r = run(&format!("G0 X1.\n{code}\nG0 X2.\nG1 X3. F100.\n"));
        assert_eq!(
            r.moves(),
            vec![
                &Command::Rapid {
                    target: vec![w(0, 1.0)]
                },
                &Command::Rapid {
                    target: vec![w(0, 2.0)]
                },
                &Command::Linear {
                    target: vec![w(0, 3.0)]
                },
            ],
            "{code}"
        );
        assert!(
            !r.commands
                .iter()
                .any(|c| matches!(c, Command::PositionUnknown { .. })),
            "{code}: nothing is hidden"
        );
    }
}

#[test]
fn each_is_reported_once_where_it_appears() {
    for code in CODES {
        let r = run(&format!("G0 X1.\n{code}\nG0 X2.\n"));
        let about: Vec<_> = r
            .diags
            .iter()
            .filter(|d| {
                matches!(
                    d.code,
                    DiagCode::UnsupportedGCode | DiagCode::UnsupportedMCode
                )
            })
            .collect();
        assert_eq!(about.len(), 1, "{code}: {:?}", r.diags);
        assert_eq!(about[0].src.line, 2, "{code}");
        // Known codes are warnings; an unknown G code is an error.
        let expected = if code.starts_with("G33") {
            Severity::Error
        } else {
            Severity::Warning
        };
        assert_eq!(about[0].severity, expected, "{code}");
    }
}

#[test]
fn cancel_codes_are_quiet() {
    let r = run("G68 X0 Y0 R90.\nG69\nG51 X0 Y0 P2\nG50\nG51.1 X0.\nG50.1\nG16\nG15\n");
    assert_eq!(r.diags.len(), 4, "{:?}", r.diags);
    assert!(r.diags.iter().all(|d| d.src.line % 2 == 1), "{:?}", r.diags);
}

#[test]
fn non_motion_commands_still_apply() {
    let r = run("G10 L2 P1 X0\nM5\nM30\n");
    assert!(r.commands.contains(&Command::ProgramEnd { rewind: true }));
}

#[test]
fn harmless_path_modes_are_quiet() {
    // Exact-stop / cutting-mode and look-ahead codes do not change the path.
    let r = run("G64\nG0 X1.\nG61 G0 X2.\nG05.1 Q1\nG0 X3.\nG09 G0 X4.\n");
    assert_eq!(r.moves().len(), 4, "{:?}", r.diags);
    assert!(r.errors().is_empty(), "{:?}", r.diags);
}
