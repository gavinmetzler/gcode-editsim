//! Subprograms in the same file (M3.7): `O1000` or `:1000` starts a
//! program, M98 calls one, M99 returns. The dialect only says what the
//! program asks for; the interpreter follows the calls.
//!
//! M98 forms: `M98 P1000 L3` (P is the program, L the repeat count), or
//! `M98 P31000` (more than four digits: the leading digits are the repeat
//! count and the last four the program, Series 0i style). *(unverified)*

mod common;

use common::run;
use dialect_api::{Command, Dialect};
use dialect_fanuc::Fanuc;
use ir::{DiagCode, Severity};

fn calls(src: &str) -> Vec<Command> {
    run(src)
        .commands
        .into_iter()
        .filter(|c| {
            matches!(
                c,
                Command::CallSubprogram { .. } | Command::ReturnFromSubprogram
            )
        })
        .collect()
}

#[test]
fn m98_p_calls_once() {
    assert_eq!(
        calls("M98 P1000\n"),
        vec![Command::CallSubprogram {
            program: 1000,
            repeat: 1
        }]
    );
}

#[test]
fn leading_digits_of_p_repeat() {
    assert_eq!(
        calls("M98 P190098\n"),
        vec![Command::CallSubprogram {
            program: 98,
            repeat: 19
        }]
    );
}

#[test]
fn l_repeats() {
    assert_eq!(
        calls("M98 P98 L3\n"),
        vec![Command::CallSubprogram {
            program: 98,
            repeat: 3
        }]
    );
}

#[test]
fn m99_returns() {
    assert_eq!(calls("M99\n"), vec![Command::ReturnFromSubprogram]);
}

#[test]
fn a_call_without_p_is_an_error() {
    let r = run("M98\n");
    assert!(r
        .commands
        .iter()
        .all(|c| !matches!(c, Command::CallSubprogram { .. })));
    let d = r
        .diags
        .iter()
        .find(|d| d.code == DiagCode::MissingWord)
        .expect("reported");
    assert_eq!(d.severity, Severity::Error);
}

#[test]
fn the_call_comes_after_the_blocks_move() {
    let r = run("G0 X1. M98 P10\n");
    let kinds: Vec<&str> = r
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Rapid { .. } => Some("move"),
            Command::CallSubprogram { .. } => Some("call"),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, vec!["move", "call"]);
}

#[test]
fn o_and_colon_start_a_program() {
    let fanuc = Fanuc::new().unwrap();
    let p = gcode_syntax::parse_str("O1000\n:0098\nG0 X1.\nO12 (NAME)\n");
    let numbers: Vec<Option<u32>> = p.blocks.iter().map(|b| fanuc.program_number(b)).collect();
    assert_eq!(numbers, vec![Some(1000), Some(98), None, Some(12)]);
}

#[test]
fn a_program_number_line_is_quiet() {
    let r = run("O1000 (MAIN)\n:0098\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
}
