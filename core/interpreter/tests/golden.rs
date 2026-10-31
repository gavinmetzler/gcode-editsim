//! End-to-end golden tests: every `tests/golden/*.nc` file is interpreted
//! with the Fanuc dialect on the generic 3-axis mill and compared with
//! `<name>.ir.json`. Files named `router_*.nc` (excerpts of real router
//! programs) run on the generic router instead, which has no travel limits
//! and every work offset at zero.
//!
//! To regenerate after a deliberate change:
//! `UPDATE_GOLDEN=1 cargo test -p interpreter --test golden`, then review the
//! diff before committing.

use std::fs;
use std::path::{Path, PathBuf};

use dialect_fanuc::Fanuc;
use interpreter::{interpret, Options};
use machine_api::{Machine, MachineDef};
use machine_mill::Mill;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

#[test]
fn golden_programs() {
    let fanuc = Fanuc::new().unwrap();
    let mill = Mill::generic().unwrap();
    let router =
        Mill::new(MachineDef::from_toml_str(machine_mill::ROUTER_EXAMPLE_DEFINITION).unwrap())
            .unwrap();
    let letters: Vec<char> = mill.axes().iter().map(|a| a.name).collect();

    let mut inputs: Vec<PathBuf> = fs::read_dir(golden_dir())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "nc"))
        .collect();
    inputs.sort();
    assert!(!inputs.is_empty());

    let mut failures = Vec::new();
    for input in &inputs {
        let name = input.file_name().unwrap().to_string_lossy().into_owned();
        let source = fs::read(input).unwrap();
        let machine = if name.starts_with("router_") {
            &router
        } else {
            &mill
        };
        let program = interpret(&source, &name, &fanuc, machine, Options::default());
        // Messages are shown to users: no stray whitespace from string
        // continuation mistakes.
        for d in &program.diagnostics {
            assert!(!d.message.contains("  "), "{name}: {:?}", d.message);
        }
        let actual = testkit::summary(&program, &letters);
        let expected_path = input.with_extension("ir.json");
        if testkit::update_requested() {
            fs::write(&expected_path, &actual).unwrap();
            continue;
        }
        let expected = fs::read_to_string(&expected_path).ok();
        if let Err(e) = testkit::compare(&name, expected.as_deref(), &actual) {
            failures.push(e);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
