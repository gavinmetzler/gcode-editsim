//! Rules about what may appear together in one block.

mod common;

use common::{axes, run, run_with, w};
use dialect_api::Command;
use dialect_fanuc::FanucOptions;
use ir::{DiagCode, Severity};

#[test]
fn last_code_of_a_modal_group_wins() {
    let r = run("G1 G0 X1.\n");
    assert_eq!(r.codes(), vec![DiagCode::ModalGroupConflict]);
    assert_eq!(r.diags[0].severity, Severity::Warning);
    assert!(r.diags[0].dialect_specific);
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 1.0)]
        }]
    );
}

#[test]
fn codes_from_different_groups_combine() {
    let r = run("G91 G1 X1. F10.\n");
    assert!(r.diags.is_empty());
}

#[test]
fn repeated_address_skips_the_block() {
    let r = run("G0 X1. X2.\n");
    assert_eq!(r.codes(), vec![DiagCode::InvalidValue]);
    assert!(r.commands.is_empty());
}

#[test]
fn several_m_codes_warn_once_and_all_apply() {
    let r = run("M3 M8\nM5 M9\n");
    assert_eq!(r.codes(), vec![DiagCode::ControllerDependent]);
    assert_eq!(r.commands.len(), 4);
}

#[test]
fn words_with_no_effect_are_reported() {
    let r = run("G0 X1. Q5.\nG1 X2. R3. F10.\n");
    let found: Vec<(u32, DiagCode)> = r.diags.iter().map(|d| (d.src.line, d.code)).collect();
    assert_eq!(
        found,
        vec![
            (1, DiagCode::UnsupportedWord),
            (2, DiagCode::UnsupportedWord)
        ]
    );
    assert_eq!(r.moves().len(), 2);
}

#[test]
fn axis_letter_the_machine_lacks_is_reported() {
    let r = run("G0 X1. A90.\n");
    assert_eq!(r.codes(), vec![DiagCode::UnsupportedWord]);
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 1.0)]
        }]
    );
    // The same letter is a real axis on a machine that has it.
    let machine = axes(&[('X', Some('I'), false), ('A', None, true)]);
    let r = run_with(FanucOptions::default(), &machine, "G0 X1. A90.\n");
    assert!(r.diags.is_empty());
}

#[test]
fn program_number_and_sequence_numbers_are_accepted() {
    let r = run("O1000\nN10 G0 X1.\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
}

#[test]
fn syntax_errors_skip_the_block() {
    let r = run("G0 X\nG0 X2.\n");
    assert_eq!(r.codes(), vec![DiagCode::SyntaxError]);
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 2.0)]
        }]
    );
}

#[test]
fn macro_expressions_skip_the_block() {
    let r = run("G0 X#100\nG0 X[1+2]\n");
    assert_eq!(
        r.codes(),
        vec![DiagCode::NotYetSimulated, DiagCode::NotYetSimulated]
    );
    assert!(r.errors().len() == 2);
    assert!(r.commands.is_empty());
}

#[test]
fn macro_assignments_warn() {
    let r = run("#100 = 5.\nG0 X1.\n");
    assert_eq!(r.codes(), vec![DiagCode::NotYetSimulated]);
    assert_eq!(r.diags[0].severity, Severity::Warning);
    assert_eq!(r.moves().len(), 1);
}

#[test]
fn diagnostics_point_at_the_offending_word() {
    let r = run("G0 X1. M50\n");
    let d = &r.diags[0];
    assert_eq!((d.src.line, d.src.col_start, d.src.col_end), (1, 7, 10));
}
