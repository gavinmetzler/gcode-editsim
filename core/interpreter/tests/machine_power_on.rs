//! M5.6b: the power-on state a machine file states reaches the dialect, so
//! programs that rely on it are not warned about assumed modes.

use dialect_api::Dialect;
use dialect_fanuc::Fanuc;
use dialect_osai::Osai;
use interpreter::{interpret, Options};
use machine_api::MachineDef;
use machine_mill::Mill;

fn machine(power_on: &str) -> Mill {
    let text = format!(
        "[machine]\nname = \"m\"\nfamily = \"mill\"\n{power_on}\n\
         [[axis]]\nletter = \"X\"\nkind = \"linear\"\narc_offset = \"I\"\n\
         [[axis]]\nletter = \"Y\"\nkind = \"linear\"\narc_offset = \"J\"\n\
         [[axis]]\nletter = \"Z\"\nkind = \"linear\"\narc_offset = \"K\"\n"
    );
    Mill::new(MachineDef::from_toml_str(&text).expect("valid")).expect("valid")
}

const STATED: &str =
    "[power_on]\nunits = \"mm\"\nplane = [\"X\", \"Y\"]\nmotion = \"rapid\"\ndistance = \"absolute\"";

fn assumed(dialect: &dyn Dialect, power_on: &str, src: &str) -> Vec<String> {
    let p = interpret(
        src.as_bytes(),
        "t.nc",
        dialect,
        &machine(power_on),
        Options::default(),
    );
    p.diagnostics
        .iter()
        .filter(|d| d.message.starts_with("assumed "))
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn fanuc_programs_on_a_machine_that_states_its_power_on_state_get_no_assumed_warnings() {
    let src = "X10. Y10.\nG2 X20. R5. F100.\n";
    let fanuc = Fanuc::new().expect("loads");
    assert!(!assumed(&fanuc, "", src).is_empty());
    assert_eq!(assumed(&fanuc, STATED, src), Vec::<String>::new());
}

#[test]
fn osai_programs_on_a_machine_that_states_its_power_on_state_get_no_assumed_warnings() {
    let src = "G0 X10 Y10\nG2 X20 Y10 I15 J10 F100\n";
    let osai = Osai::new().expect("loads");
    assert!(!assumed(&osai, "", src).is_empty());
    assert_eq!(assumed(&osai, STATED, src), Vec::<String>::new());
}
