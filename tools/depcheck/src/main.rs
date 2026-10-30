//! Runs the dependency check on this workspace.
//!
//! `cargo run -p depcheck` exits with status 1 and lists every violation
//! if a crate depends on something ARCHITECTURE.md section 5 forbids.

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = match Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            eprintln!(
                "cargo metadata failed:\n{}",
                String::from_utf8_lossy(&o.stderr)
            );
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("cannot run cargo metadata: {e}");
            return ExitCode::FAILURE;
        }
    };
    let metadata: depcheck::Metadata = match serde_json::from_slice(&output.stdout) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("cannot read cargo metadata output: {e}");
            return ExitCode::FAILURE;
        }
    };
    let rules = match depcheck::Rules::parse(depcheck::RULES) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("invalid tools/depcheck/rules.toml: {e}");
            return ExitCode::FAILURE;
        }
    };
    let violations = depcheck::check(&metadata, &rules);
    if violations.is_empty() {
        println!(
            "dependency rules: OK ({} crates checked)",
            metadata.packages.len()
        );
        return ExitCode::SUCCESS;
    }
    eprintln!("dependency rules violated:");
    for v in &violations {
        eprintln!("  - {v}");
    }
    ExitCode::FAILURE
}
