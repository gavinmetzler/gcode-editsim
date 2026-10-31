//! Subprograms in the same file (M3.7): the interpreter follows M98 calls
//! to the program the dialect numbers, runs it the asked number of times,
//! and returns after the call at M99. Segments keep the lines they came
//! from, so a subprogram line has segments from every call.

mod common;

use common::{at, codes, moves, run, run_opts};
use interpreter::Options;
use ir::{DiagCode, Severity};

/// Main program at G54 (X-380 Y-205 Z-400): two calls of O10, which steps
/// X by +10 incrementally, then a move to X50.
const TWICE: &str = "G21 G90 G17 G54 G0 X0. Y0. Z5.\n\
                     M98 P10 L2\n\
                     G0 X50.\n\
                     M30\n\
                     O10\n\
                     G91 G0 X10.\n\
                     G90\n\
                     M99\n";

#[test]
fn a_call_runs_the_subprogram_and_returns_after_it() {
    let p = run(TWICE);
    let m = moves(&p);
    let lines: Vec<u32> = m.iter().map(|s| s.src.line).collect();
    assert_eq!(lines, vec![1, 6, 6, 3]);
    at(&m[1].end, [-370.0, -205.0, -395.0]);
    at(&m[2].end, [-360.0, -205.0, -395.0]);
    at(&m[3].end, [-330.0, -205.0, -395.0]);
    assert!(codes(&p).is_empty(), "{:?}", p.diagnostics);
}

#[test]
fn leading_digits_of_p_repeat_the_call() {
    let p = run("G21 G90 G17 G54 G0 X0. Y0. Z5.\nM98 P30010\nM30\nO10\nG91 G0 X10.\nG90\nM99\n");
    let m = moves(&p);
    assert_eq!(m.len(), 4);
    at(&m[3].end, [-350.0, -205.0, -395.0]);
}

#[test]
fn calls_nest() {
    let p = run("G21 G90 G17 G54 G0 X0. Y0. Z5.\nM98 P10\nM30\n\
         O10\nG91 G0 X10.\nM98 P20 L2\nG90\nM99\n\
         O20\nG91 G0 Y-1.\nG90\nM99\n");
    let lines: Vec<u32> = moves(&p).iter().map(|s| s.src.line).collect();
    assert_eq!(lines, vec![1, 5, 10, 10]);
    at(&moves(&p)[3].end, [-370.0, -207.0, -395.0]);
}

#[test]
fn a_program_not_in_the_file_is_skipped_with_a_warning() {
    let p = run("G21 G90 G17 G54 G0 X0. Y0. Z5.\nM98 P77\nG0 X5.\nM30\n");
    assert_eq!(moves(&p).len(), 2);
    let d = p
        .diagnostics
        .iter()
        .find(|d| d.message.contains("O77"))
        .expect("reported");
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.src.line, 2);
}

#[test]
fn a_program_calling_itself_stops_at_the_depth_limit() {
    let p = run("G21 G90 G17 G54 G0 X0. Y0. Z5.\nM98 P10\nG0 X5.\nM30\nO10\nG91 G0 X1.\nG90\nM98 P10\nM99\n");
    // Ten levels deep, then the call is refused and everything unwinds.
    assert_eq!(moves(&p).len(), 1 + 10 + 1);
    let errors: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].message.contains("deep"), "{}", errors[0].message);
}

#[test]
fn m99_in_the_main_program_ends_it() {
    let p = run("G21 G90 G17 G54 G0 X0. Y0. Z5.\nG0 X5.\nM99\nG0 X9.\n");
    assert_eq!(moves(&p).len(), 2);
    assert!(p
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Info && d.message.contains("M99 in the main program")));
}

#[test]
fn the_main_program_ends_where_another_program_starts() {
    // No M30: running on into O10 would draw the subprogram as if called.
    let p = run("G21 G90 G17 G54 G0 X0. Y0. Z5.\nG0 X5.\nO10\nG0 X9.\nM99\n");
    assert_eq!(moves(&p).len(), 2);
    assert!(p
        .diagnostics
        .iter()
        .any(|d| d.src.line == 3 && d.message.contains("O10")));
}

#[test]
fn the_main_programs_own_number_is_not_an_end() {
    let p =
        run("%\nO1000 (MAIN)\nG21 G90 G17 G54 G0 X0. Y0. Z5.\nM98 P10\nM30\nO10\nG0 X1.\nM99\n%\n");
    let lines: Vec<u32> = moves(&p).iter().map(|s| s.src.line).collect();
    assert_eq!(lines, vec![3, 7]);
}

#[test]
fn diagnostics_from_a_repeated_subprogram_are_reported_once() {
    let p = run(
        "G21 G90 G17 G54 G0 X0. Y0. Z5.\nM98 P10 L3\nM30\nO10\nG68 X0. Y0. R45.\nG0 X1.\nM99\n",
    );
    let rotation: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagCode::UnsupportedGCode)
        .collect();
    assert_eq!(rotation.len(), 1, "{rotation:?}");
}

#[test]
fn code_after_the_end_that_is_never_called_is_noted() {
    let p = run("G21 G90 G17 G54 G0 X0. Y0. Z5.\nM30\nG0 X9.\nO10\nG0 X1.\nM99\n");
    let note = p
        .diagnostics
        .iter()
        .find(|d| d.message.contains("not run"))
        .expect("noted");
    assert_eq!((note.severity, note.src.line), (Severity::Info, 3));
}

#[test]
fn a_runaway_program_stops_at_the_block_limit() {
    let p = run_opts(
        "G21 G90 G17 G54 G0 X0. Y0. Z5.\nM98 P10 L9999\nM30\nO10\nG0 X1.\nG0 X2.\nM99\n",
        Options {
            max_blocks: 100,
            ..Options::default()
        },
    );
    assert!(moves(&p).len() < 100);
    assert!(p
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error && d.message.contains("100 blocks")));
}

#[test]
fn code_after_the_end_of_a_numbered_main_program_is_noted() {
    // The main program has a number of its own: code after its M30 is
    // still after the end.
    let p = run("O1000\nG21 G90 G17 G54 G0 X0. Y0. Z5.\nM30\nG0 X9.\n");
    let note = p
        .diagnostics
        .iter()
        .find(|d| d.message.contains("not run"))
        .expect("noted");
    assert_eq!(note.src.line, 4);
}
