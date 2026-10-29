//! Golden tests: every `tests/golden/*.nc` file is parsed and compared with
//! the reviewed expected output in `<name>.syntax.txt` next to it.
//!
//! To regenerate the expected files after a deliberate change, run
//! `UPDATE_GOLDEN=1 cargo test -p gcode-syntax --test golden` and review the
//! diff before committing.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use gcode_syntax::{parse, CommentKind, Expr, Program};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

/// A word value such as `ATAN[1, 2]` is only valid G-code inside brackets.
fn needs_brackets(e: &Expr) -> bool {
    match e {
        Expr::Call { .. } => true,
        Expr::Neg(inner) => needs_brackets(inner),
        _ => false,
    }
}

/// A stable, human-readable rendering of a parse result.
fn render(program: &Program) -> String {
    let mut out = String::new();
    for b in &program.blocks {
        let _ = write!(out, "line {}:", b.line);
        if b.program_delimiter {
            out.push_str(" %");
        }
        if let Some(level) = b.block_skip {
            let _ = write!(out, " skip={level}");
        }
        if let Some(n) = b.sequence_number {
            let _ = write!(out, " N{n}");
        }
        out.push('\n');
        if !b.words.is_empty() {
            out.push_str("  words:");
            for w in &b.words {
                if needs_brackets(&w.value) {
                    let _ = write!(out, " {}[{}]", w.letter, w.value);
                } else {
                    let _ = write!(out, " {}{}", w.letter, w.value);
                }
            }
            out.push('\n');
        }
        for a in &b.assignments {
            let _ = writeln!(out, "  assign: {} = {}", a.target, a.value);
        }
        for c in &b.comments {
            let kind = match c.kind {
                CommentKind::Paren => "paren",
                CommentKind::Semicolon => "semicolon",
            };
            let _ = writeln!(out, "  comment ({kind}): {:?}", c.text);
        }
        for e in &b.errors {
            let _ = writeln!(out, "  error @{}..{}: {}", e.span.start, e.span.end, e);
        }
    }
    out
}

#[test]
fn golden_files_match() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut inputs: Vec<PathBuf> = fs::read_dir(golden_dir())
        .expect("tests/golden exists")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "nc"))
        .collect();
    inputs.sort();
    assert!(!inputs.is_empty(), "no golden inputs found");

    let mut failures = Vec::new();
    for input in &inputs {
        let src = fs::read(input).expect("read golden input");
        let actual = render(&parse(&src));
        let expected_path = input.with_extension("syntax.txt");
        if update {
            fs::write(&expected_path, &actual).expect("write golden output");
            continue;
        }
        match fs::read_to_string(&expected_path) {
            Ok(expected) if expected.replace("\r\n", "\n") == actual => {}
            Ok(expected) => failures.push(format!(
                "{} differs.\n--- expected\n{expected}\n--- actual\n{actual}",
                input.display()
            )),
            Err(_) => failures.push(format!(
                "{} has no expected output; run with UPDATE_GOLDEN=1 and review it.\n{actual}",
                input.display()
            )),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
