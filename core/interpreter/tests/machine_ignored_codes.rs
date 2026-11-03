//! Codes a machine file lists in `ignore_codes` give no message (M5.6c,
//! owner, 2026-11-03): M codes (the machine builder's head selection,
//! clamps) are removed from every block before the dialect sees them; G
//! codes and Osai three-letter commands keep their behaviour and only lose
//! their "not supported" message. Codes nobody listed are still reported.

use dialect_api::Dialect;
use dialect_fanuc::Fanuc;
use dialect_osai::Osai;
use interpreter::{interpret, Options};
use ir::{DiagCode, SegKind};
use machine_api::MachineDef;
use machine_mill::Mill;

fn run(src: &str, ignore: &str) -> ir::Program {
    run_with(&Fanuc::new().unwrap(), src, ignore)
}

fn run_with(dialect: &dyn Dialect, src: &str, ignore: &str) -> ir::Program {
    let text = format!(
        "[machine]\nname = \"m\"\nfamily = \"mill\"\nignore_codes = [{ignore}]\n\
         [[axis]]\nletter = \"X\"\nkind = \"linear\"\narc_offset = \"I\"\n\
         [[axis]]\nletter = \"Y\"\nkind = \"linear\"\narc_offset = \"J\"\n\
         [[axis]]\nletter = \"Z\"\nkind = \"linear\"\narc_offset = \"K\"\n"
    );
    let machine = Mill::new(MachineDef::from_toml_str(&text).unwrap()).unwrap();
    interpret(
        src.as_bytes(),
        "t.nc",
        dialect,
        &machine,
        Options::default(),
    )
}

fn moves(p: &ir::Program) -> usize {
    p.channels[0]
        .segments
        .iter()
        .filter(|s| matches!(s.kind, SegKind::Rapid | SegKind::Linear))
        .count()
}

#[test]
fn listed_m_codes_give_no_message_and_nothing_from_the_dialect() {
    let src = "G21 G90 G17\nM92\nG0 X1. M213\nM92\nM213\n";
    let p = run(src, "\"M92\", \"M213\"");
    let mentions: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("M92") || d.message.contains("M213"))
        .collect();
    assert_eq!(mentions, Vec::<&ir::Diagnostic>::new());
    assert_eq!(moves(&p), 1);
    // Not listed: each still gets its note.
    let unlisted = run(src, "");
    let about: Vec<_> = unlisted
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagCode::UnsupportedMCode)
        .collect();
    assert_eq!(about.len(), 2, "{about:?}");
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
    assert!(
        p.diagnostics.iter().all(|d| !d.message.contains("M92")),
        "{:?}",
        p.diagnostics
    );
}

fn about(p: &ir::Program, word: &str) -> Vec<String> {
    p.diagnostics
        .iter()
        .filter(|d| d.message.contains(word))
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn a_listed_g43_gives_no_note_and_its_move_is_still_drawn() {
    let src = "G21 G90 G17 G0 X0. Y0. Z0.\nG91 G43 Z-40. H11\n";
    let unlisted = run(src, "");
    assert_eq!(
        about(&unlisted, "G43").len(),
        1,
        "{:?}",
        unlisted.diagnostics
    );
    let listed = run(src, "\"G43\"");
    assert_eq!(about(&listed, "G43"), Vec::<String>::new());
    let last = listed.channels[0].segments.last().expect("a move");
    assert_eq!(last.end[2], -40.0);
    assert_eq!(moves(&listed), moves(&unlisted));
}

#[test]
fn a_listed_g_code_loses_its_not_simulated_warning() {
    // Cutter compensation, for example, until M7 simulates it.
    let src = "G21 G90 G17 G0 X0. Y0.\nG42 G1 X10. D1 F100.\nG40 X20.\n";
    assert!(!about(&run(src, ""), "Cutter compensation").is_empty());
    assert_eq!(
        about(&run(src, "\"G42\""), "Cutter compensation"),
        Vec::<String>::new()
    );
}

#[test]
fn unknown_codes_nobody_listed_are_still_reported() {
    let p = run("G21 G90 G17\nG0 X1.\nG011 X2.\nM777\n", "\"G43\", \"M92\"");
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.code == DiagCode::UnsupportedGCode),
        "{:?}",
        p.diagnostics
    );
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.code == DiagCode::UnsupportedMCode),
        "{:?}",
        p.diagnostics
    );
}

#[test]
fn a_listed_osai_uao_gives_no_warning() {
    let osai = Osai::new().unwrap();
    let src = "G71 G17 G90\n(UAO,1,Z)\nG0 X1 Y1 Z1\n";
    assert_eq!(about(&run_with(&osai, src, ""), "UAO").len(), 1);
    assert_eq!(
        about(&run_with(&osai, src, "\"UAO\""), "UAO"),
        Vec::<String>::new()
    );
}

#[test]
fn a_listed_osai_dwt_silences_the_dwell_time_warning() {
    // The message is on the G04 block: it is about DWT, which is not read.
    let osai = Osai::new().unwrap();
    let src = "G71 G17 G90\n(DWT,2)\nG04\nG0 X1 Y1 Z1\n";
    // Unlisted: the G04 dwell and the (DWT,2) command itself are reported.
    assert!(!about(&run_with(&osai, src, ""), "DWT").is_empty());
    assert_eq!(
        about(&run_with(&osai, src, "\"DWT\""), "DWT"),
        Vec::<String>::new()
    );
}
