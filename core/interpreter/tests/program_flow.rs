//! Program end and block delete.

mod common;

use common::{codes, moves, run, run_opts};
use interpreter::Options;
use ir::{DiagCode, Severity};

#[test]
fn blocks_after_program_end_are_not_executed() {
    let p = run("G0 X1.\nM30\nG0 X2.\nG0 X3.\n");
    assert_eq!(moves(&p).len(), 1);
    assert_eq!(codes(&p), vec![DiagCode::NotYetSimulated]);
    assert_eq!(p.diagnostics[0].severity, Severity::Info);
    assert_eq!(p.diagnostics[0].src.line, 3);
}

#[test]
fn trailing_percent_and_comments_after_end_are_quiet() {
    let p = run("%\nG0 X1.\nM30\n(end)\n%\n");
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
}

#[test]
fn block_delete_switch() {
    let src = "G0 X1.\n/G0 X2.\n/2 G0 X3.\nG0 X4.\n";
    assert_eq!(moves(&run(src)).len(), 4);
    let skipped = run_opts(
        src,
        Options {
            block_delete: true,
            ..Options::default()
        },
    );
    assert_eq!(moves(&skipped).len(), 2);
}
