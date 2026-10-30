//! Interprets a G-code file with the Fanuc dialect and prints the result
//! as JSON: segments in machine coordinates, diagnostics and bounding box.
//!
//! ```text
//! cargo run -p interpreter --example summary -- program.nc [machine.toml]
//! ```
//!
//! Without a machine file, the generic 3-axis mill shipped with
//! `machine-mill` is used.

use std::process::ExitCode;

use dialect_fanuc::Fanuc;
use interpreter::{interpret, Options};
use machine_api::Machine;
use machine_mill::Mill;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(program_path) = args.first() else {
        eprintln!("usage: summary <program.nc> [machine.toml]");
        return ExitCode::FAILURE;
    };
    let source = match std::fs::read(program_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {program_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let machine = match args.get(1) {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {path}: {e}"))
            .and_then(|text| Mill::from_toml_str(&text).map_err(|e| format!("{path}: {e}"))),
        None => Mill::generic().map_err(|e| e.to_string()),
    };
    let machine = match machine {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let dialect = match Fanuc::new() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let program = interpret(
        &source,
        program_path,
        &dialect,
        &machine,
        Options::default(),
    );
    let letters: Vec<char> = machine.axes().iter().map(|a| a.name).collect();
    print!("{}", testkit::summary(&program, &letters));
    ExitCode::SUCCESS
}
