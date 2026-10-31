//! M codes a machine file lists in `ignore_codes` are removed from every
//! block before the dialect sees them, with one note per code. They are the
//! machine builder's codes (head selection, clamps), which the user knows
//! do not move the axes. Only M codes can be listed (machine-api rejects G
//! codes, whose axis words would be left behind and read as a move).

use dialect_fanuc::Fanuc;
use interpreter::{interpret, Options};
use ir::{DiagCode, SegKind, Severity};
use machine_api::MachineDef;
use machine_mill::Mill;

fn run(src: &str, ignore: &str) -> ir::Program {
    let text = format!(
        "[machine]\nname = \"m\"\nfamily = \"mill\"\nignore_codes = [{ignore}]\n\
         [[axis]]\nletter = \"X\"\nkind = \"linear\"\narc_offset = \"I\"\n\
         [[axis]]\nletter = \"Y\"\nkind = \"linear\"\narc_offset = \"J\"\n\
         [[axis]]\nletter = \"Z\"\nkind = \"linear\"\narc_offset = \"K\"\n"
    );
    let machine = Mill::new(MachineDef::from_toml_str(&text).unwrap()).unwrap();
    let fanuc = Fanuc::new().unwrap();
    interpret(src.as_bytes(), "t.nc", &fanuc, &machine, Options::default())
}

fn moves(p: &ir::Program) -> usize {
    p.channels[0]
        .segments
        .iter()
        .filter(|s| matches!(s.kind, SegKind::Rapid | SegKind::Linear))
        .count()
}

#[test]
fn listed_codes_get_one_note_each_and_nothing_from_the_dialect() {
    let p = run(
        "G21 G90 G17\nM92\nG0 X1. M213\nM92\nM213\n",
        "\"M92\", \"M213\"",
    );
    let about: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("M92") || d.message.contains("M213"))
        .collect();
    assert_eq!(about.len(), 2, "{about:?}");
    assert!(about.iter().all(|d| d.severity == Severity::Info
        && d.code == DiagCode::UnsupportedMCode
        && d.message.contains("ignore_codes")));
    assert_eq!((about[0].src.line, about[1].src.line), (2, 3));
    assert_eq!(moves(&p), 1);
}

#[test]
fn the_rest_of_the_block_still_runs() {
    let p = run("G21 G90 G17\nG0 X5. M92 Y6.\n", "\"M92\"");
    let s = &p.channels[0].segments;
    let last = s.iter().rev().find(|s| s.kind == SegKind::Rapid).unwrap();
    assert_eq!((last.end[0], last.end[1]), (5.0, 6.0));
}

#[test]
fn forms_like_m092_match() {
    let p = run("G21 G90 G17\nM092\nM92.0\n", "\"M92\"");
    assert!(p.diagnostics.iter().all(|d| d.severity == Severity::Info));
    assert_eq!(p.diagnostics.len(), 1, "{:?}", p.diagnostics);
}
