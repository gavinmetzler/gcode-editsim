//! M5.6 wrap-up (owner, 2026-11-03): on a machine file that describes the
//! machine (its `ignore_codes` and `[power_on]` state), programs written
//! the way the owner's routers are programmed give no "not supported"
//! messages. What is left is useful: one warning for cutter compensation
//! (the path is drawn without it) and a note for M6 with no tool.
//!
//! The programs here are written for this test in the style of the owner's
//! programs (which are private and never copied into the repository).

use dialect_api::Dialect;
use dialect_fanuc::Fanuc;
use dialect_osai::Osai;
use interpreter::{interpret, Options};
use machine_api::MachineDef;
use machine_mill::Mill;

fn router(dialect: &str, ignore: &str) -> Mill {
    let text = format!(
        "[machine]\nname = \"router\"\nfamily = \"mill\"\ndialect = \"{dialect}\"\n\
         ignore_codes = [{ignore}]\n\
         [power_on]\nunits = \"mm\"\nplane = [\"X\", \"Y\"]\nmotion = \"rapid\"\n\
         distance = \"absolute\"\n\
         [[axis]]\nletter = \"X\"\nkind = \"linear\"\narc_offset = \"I\"\nreference = 0.0\n\
         [[axis]]\nletter = \"Y\"\nkind = \"linear\"\narc_offset = \"J\"\nreference = 0.0\n\
         [[axis]]\nletter = \"Z\"\nkind = \"linear\"\narc_offset = \"K\"\nreference = 0.0\n\
         [work_offsets]\n1 = {{}}\n2 = {{ X = 500.0 }}\n3 = {{ X = 1000.0 }}\n"
    );
    Mill::new(MachineDef::from_toml_str(&text).expect("valid")).expect("valid")
}

/// Every diagnostic, as "severity: message".
fn messages(dialect: &dyn Dialect, machine: &Mill, src: &str) -> Vec<String> {
    let p = interpret(src.as_bytes(), "t.nc", dialect, machine, Options::default());
    p.diagnostics
        .iter()
        .map(|d| format!("{:?}: {}", d.severity, d.message))
        .collect()
}

const FANUC_ROUTER: &str = "%
:0100
(PANEL, 2 SHEETS)
G54 G92 X1000. Y1250. Z0.
M06
M211 M40
G55
G90 X0. Y0.
G43 Z25. H1
M13 S18000 M03
G01 Z-5. F3000.
G01 X300. F6000.
G02 X350. Y50. R50.
G01 Y400.
G42 G01 X400. D1
G01 Y600.
G40 G01 X450.
G00 Z25.
M213 M95
G56
X0. Y0.
G28 G91 Z0.
M30
%
";

#[test]
fn a_fanuc_router_program_keeps_only_the_useful_messages() {
    let fanuc = Fanuc::new().expect("loads");
    let machine = router(
        "fanuc",
        "\"M13\", \"M40\", \"M95\", \"M211\", \"M213\", \"G43\"",
    );
    let got = messages(&fanuc, &machine, FANUC_ROUTER);
    let warnings: Vec<_> = got.iter().filter(|m| m.starts_with("Warning")).collect();
    let errors: Vec<_> = got.iter().filter(|m| m.starts_with("Error")).collect();
    let notes: Vec<_> = got.iter().filter(|m| m.starts_with("Info")).collect();
    assert_eq!(errors, Vec::<&String>::new());
    assert_eq!(warnings.len(), 1, "{got:#?}");
    assert!(warnings[0].contains("G42"), "{got:#?}");
    assert_eq!(notes.len(), 1, "{got:#?}");
    assert!(notes[0].contains("M6 with no tool"), "{got:#?}");
}

const OSAI_ROUTER: &str = "(UAO,1)
(UAO,1,Z)
(DWT,1)
G0 X0 Y0 Z25
M3 S18000
G04
G1 Z-5 F3000
G1 X300 F6000
G2 X350 Y50 I300 J50
G1 Y400
G92 X0 Y0
G0 Z25
G1 X100 Y100
G99
(UAO,2)
G0 X0 Y0
M30
";

#[test]
fn an_osai_router_program_gives_no_messages() {
    let osai = Osai::new().expect("loads");
    let machine = router("osai", "\"UAO\", \"DWT\"");
    assert_eq!(messages(&osai, &machine, OSAI_ROUTER), Vec::<String>::new());
}

#[test]
fn without_the_machine_file_the_same_programs_say_what_is_not_simulated() {
    // The messages are silenced by the machine file, not lost: an empty
    // `ignore_codes` reports them again.
    let fanuc = Fanuc::new().expect("loads");
    let got = messages(&fanuc, &router("fanuc", ""), FANUC_ROUTER);
    assert!(got.iter().any(|m| m.contains("G43")), "{got:#?}");
    assert!(got.iter().any(|m| m.contains("M211")), "{got:#?}");
    let osai = Osai::new().expect("loads");
    let got = messages(&osai, &router("osai", ""), OSAI_ROUTER);
    assert!(got.iter().any(|m| m.contains("UAO")), "{got:#?}");
    assert!(got.iter().any(|m| m.contains("DWT")), "{got:#?}");
}
