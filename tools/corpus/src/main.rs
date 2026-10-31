//! Runs real G-code programs through the whole pipeline and reports, per
//! file, how much was drawn and every diagnostic it gave (grouped, with a
//! count and the first line).
//!
//! The owner's real programs are private and stay out of this repository
//! (M3.0, `docs/plans/M3_PLAN.md`); this tool runs them locally:
//!
//! ```text
//! cargo run -p corpus -- --dialect fanuc [--machine FILE.toml] FILE...
//! ```
//!
//! Without `--machine`, the shipped generic router is used (no travel
//! limits, every work offset at zero).

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use ir::{Diagnostic, Program, SegFlags, SegKind, Severity};

/// What to run.
#[derive(Debug, PartialEq)]
struct Args {
    dialect: String,
    machine: Option<PathBuf>,
    files: Vec<PathBuf>,
}

/// Reads the command line (without the program name).
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut dialect = None;
    let mut machine = None;
    let mut files = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dialect" => dialect = Some(it.next().ok_or("--dialect needs a value")?),
            "--machine" => {
                machine = Some(PathBuf::from(it.next().ok_or("--machine needs a file")?))
            }
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            _ => files.push(PathBuf::from(a)),
        }
    }
    let dialect = dialect.ok_or("--dialect fanuc|osai is required")?;
    if files.is_empty() {
        return Err("no program files given".into());
    }
    Ok(Args {
        dialect,
        machine,
        files,
    })
}

/// How much of a program was drawn.
#[derive(Debug, PartialEq)]
struct Drawn {
    segments: usize,
    moves: usize,
    position_unknown: usize,
    out_of_limits: usize,
    /// The last source line with a move, or 0.
    last_move_line: u32,
}

fn drawn(p: &Program) -> Drawn {
    let segs = p.channels.iter().flat_map(|c| c.segments.iter());
    let mut d = Drawn {
        segments: 0,
        moves: 0,
        position_unknown: 0,
        out_of_limits: 0,
        last_move_line: 0,
    };
    for s in segs {
        d.segments += 1;
        if matches!(
            s.kind,
            SegKind::Rapid | SegKind::Linear | SegKind::ArcCw | SegKind::ArcCcw
        ) {
            d.moves += 1;
            d.last_move_line = d.last_move_line.max(s.src.line);
        }
        if s.flags.contains(SegFlags::POSITION_UNKNOWN) {
            d.position_unknown += 1;
        }
        if s.flags.contains(SegFlags::OUT_OF_LIMITS) {
            d.out_of_limits += 1;
        }
    }
    d
}

/// One group of identical diagnostics.
#[derive(Debug, PartialEq)]
struct Group {
    severity: Severity,
    count: usize,
    first_line: u32,
    message: String,
}

/// Groups identical diagnostics (same severity, code and message): errors
/// first, then warnings, then notes, most frequent first within each.
fn group(diags: &[Diagnostic]) -> Vec<Group> {
    let mut map: BTreeMap<(u8, String), Group> = BTreeMap::new();
    for d in diags {
        let rank = match d.severity {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
        };
        let key = (rank, format!("{:?}: {}", d.code, d.message));
        map.entry(key)
            .and_modify(|g| g.count += 1)
            .or_insert_with(|| Group {
                severity: d.severity,
                count: 1,
                first_line: d.src.line,
                message: format!("{:?}: {}", d.code, d.message),
            });
    }
    let mut groups: Vec<(u8, Group)> = map.into_iter().map(|((r, _), g)| (r, g)).collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.count.cmp(&a.1.count)));
    groups.into_iter().map(|(_, g)| g).collect()
}

fn run(args: &Args) -> Result<(), String> {
    let machine_text = match &args.machine {
        Some(path) => app_backend::files::read_machine_file(
            &app_backend::files::MachineSource::User(path.clone()),
        )
        .map_err(|e| e.to_string())?,
        None => app_backend::files::shipped_machine("generic_3axis_router")
            .ok_or("the shipped router is missing")?
            .to_owned(),
    };
    let machine = app_backend::load_machine(&machine_text).map_err(|e| e.to_string())?;
    let dialect = app_backend::load_dialect(&args.dialect).map_err(|e| e.to_string())?;
    println!("machine: {}, dialect: {}", machine.name(), args.dialect);
    for path in &args.files {
        let source = app_backend::files::read_program(path).map_err(|e| e.to_string())?;
        let name = path.display().to_string();
        let p = interpreter::interpret(
            &source,
            &name,
            dialect.as_ref(),
            machine.as_ref(),
            interpreter::Options::default(),
        );
        let d = drawn(&p);
        let lines = String::from_utf8_lossy(&source).lines().count();
        println!(
            "\n{name}: {lines} lines; {} moves (last on line {}), {} position unknown, {} out of limits",
            d.moves, d.last_move_line, d.position_unknown, d.out_of_limits
        );
        for g in group(&p.diagnostics) {
            println!(
                "  {:>5} x {:<7} line {:>5}  {}",
                g.count,
                format!("{:?}", g.severity),
                g.first_line,
                g.message
            );
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match parse_args(std::env::args().skip(1)).and_then(|a| run(&a)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("corpus: {e}");
            eprintln!(
                "usage: cargo run -p corpus -- --dialect fanuc|osai [--machine FILE.toml] FILE..."
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::{DiagCode, SourceRef};

    fn strings(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn reads_options_and_files() {
        let a = parse_args(strings(&[
            "--dialect",
            "osai",
            "a.nc",
            "--machine",
            "m.toml",
            "b.nc",
        ]))
        .unwrap();
        assert_eq!(
            a,
            Args {
                dialect: "osai".into(),
                machine: Some("m.toml".into()),
                files: vec!["a.nc".into(), "b.nc".into()],
            }
        );
    }

    #[test]
    fn rejects_missing_parts() {
        assert!(parse_args(strings(&["a.nc"])).is_err());
        assert!(parse_args(strings(&["--dialect", "fanuc"])).is_err());
        assert!(parse_args(strings(&["--dialect"])).is_err());
        assert!(parse_args(strings(&["--dialect", "fanuc", "--fast", "a.nc"])).is_err());
    }

    fn diag(severity: Severity, line: u32, message: &str) -> Diagnostic {
        let src = SourceRef {
            line,
            ..SourceRef::default()
        };
        match severity {
            Severity::Error => Diagnostic::error(DiagCode::InvalidValue, message, src),
            Severity::Warning => Diagnostic::warning(DiagCode::InvalidValue, message, src),
            Severity::Info => Diagnostic::info(DiagCode::InvalidValue, message, src),
        }
    }

    #[test]
    fn groups_identical_diagnostics_errors_first() {
        let g = group(&[
            diag(Severity::Info, 1, "note"),
            diag(Severity::Warning, 5, "w"),
            diag(Severity::Warning, 2, "w2"),
            diag(Severity::Warning, 9, "w"),
            diag(Severity::Error, 7, "e"),
        ]);
        let summary: Vec<(Severity, usize, u32)> = g
            .iter()
            .map(|g| (g.severity, g.count, g.first_line))
            .collect();
        assert_eq!(
            summary,
            vec![
                (Severity::Error, 1, 7),
                (Severity::Warning, 2, 5),
                (Severity::Warning, 1, 2),
                (Severity::Info, 1, 1),
            ]
        );
        assert_eq!(g[1].message, "InvalidValue: w");
    }
}
