//! Tests for loading `CodeTable`s from TOML.

use dialect_api::{CodeKey, CodeTable, CodeTableError};

const VALID: &str = r#"
[dialect]
id = "example"
variant = "basic"

[[g]]
code = "G00"
group = 1
modal = true
handler = "rapid"
description = "Rapid move"

[[g]]
code = "G02"
group = 1
modal = true
handler = "arc_cw"
words = { optional = ["I", "J", "r"] }

[[g]]
code = "G54.1"
group = 14
modal = true
handler = "work_offset_extended"
words = { required = ["P"] }

[[m]]
code = "M30"
handler = "program_end"
"#;

fn key(text: &str) -> CodeKey {
    CodeKey::parse(text).unwrap()
}

#[test]
fn loads_a_valid_table() {
    let table = CodeTable::from_toml_str(VALID).unwrap();
    assert_eq!(table.id(), "example");
    assert_eq!(table.variant(), Some("basic"));
    assert_eq!(table.len(), 4);

    let g0 = table.get(key("G0")).unwrap();
    assert_eq!(g0.group, Some(1));
    assert!(g0.modal);
    assert_eq!(g0.handler, "rapid");
    assert_eq!(g0.description.as_deref(), Some("Rapid move"));

    let g2 = table.get(key("G2")).unwrap();
    assert_eq!(g2.optional_words, vec!['I', 'J', 'R']);
    assert!(g2.required_words.is_empty());

    assert_eq!(table.get(key("G54.1")).unwrap().required_words, vec!['P']);

    let m30 = table.get(key("M30")).unwrap();
    assert!(!m30.modal);
    assert_eq!(m30.group, None);

    assert!(table.get(key("G1")).is_none());
}

#[test]
fn lookup_works_from_parsed_word_values() {
    let table = CodeTable::from_toml_str(VALID).unwrap();
    // `G02` in a program parses to the number 2.0.
    let program = gcode_syntax::parse_str("G02 X1");
    let word = &program.blocks[0].words[0];
    let value = dialect_api::literal(&word.value).unwrap().value;
    let code = CodeKey::from_value(word.letter, value).unwrap();
    assert_eq!(table.get(code).unwrap().handler, "arc_cw");
}

#[test]
fn iterates_in_code_order() {
    let table = CodeTable::from_toml_str(VALID).unwrap();
    let codes: Vec<String> = table.iter().map(|d| d.code.to_string()).collect();
    assert_eq!(codes, ["G0", "G2", "G54.1", "M30"]);
}

#[test]
fn empty_table_is_allowed() {
    let table = CodeTable::from_toml_str("[dialect]\nid = \"empty\"\n").unwrap();
    assert!(table.is_empty());
    assert_eq!(table.variant(), None);
}

fn load_err(body: &str) -> CodeTableError {
    let text = format!("[dialect]\nid = \"t\"\n{body}");
    CodeTable::from_toml_str(&text).unwrap_err()
}

#[test]
fn rejects_bad_toml_and_unknown_fields() {
    assert!(matches!(
        CodeTable::from_toml_str("not toml ["),
        Err(CodeTableError::Toml(_))
    ));
    assert!(matches!(
        CodeTable::from_toml_str("[[g]]\ncode = \"G0\"\nhandler = \"x\""),
        Err(CodeTableError::Toml(_))
    ));
    assert!(matches!(
        load_err("[[g]]\ncode = \"G0\"\nhandler = \"x\"\nmodel = true"),
        CodeTableError::Toml(_)
    ));
    assert!(matches!(
        load_err("[[g]]\ncode = \"G0\""),
        CodeTableError::Toml(_)
    ));
}

#[test]
fn rejects_invalid_codes() {
    assert!(matches!(
        load_err("[[g]]\ncode = \"G-1\"\nhandler = \"x\""),
        CodeTableError::InvalidCode(c) if c == "G-1"
    ));
}

#[test]
fn rejects_code_in_wrong_section() {
    let err = load_err("[[g]]\ncode = \"M3\"\nhandler = \"x\"");
    assert!(matches!(err, CodeTableError::WrongSection { ref code, section: 'g' } if code == "M3"));
    assert_eq!(err.to_string(), "M3 is listed under [[g]]");
}

#[test]
fn rejects_duplicates_including_equivalent_spellings() {
    let err =
        load_err("[[g]]\ncode = \"G1\"\nhandler = \"a\"\n[[g]]\ncode = \"G01\"\nhandler = \"b\"");
    assert!(matches!(err, CodeTableError::Duplicate(c) if c == "G1"));
}

#[test]
fn rejects_modal_code_without_group() {
    let err = load_err("[[g]]\ncode = \"G1\"\nmodal = true\nhandler = \"a\"");
    assert!(matches!(err, CodeTableError::ModalWithoutGroup(c) if c == "G1"));
}

#[test]
fn rejects_bad_word_letters() {
    for bad in ["\"XY\"", "\"\"", "\"1\""] {
        let err = load_err(&format!(
            "[[g]]\ncode = \"G1\"\nhandler = \"a\"\nwords = {{ optional = [{bad}] }}"
        ));
        assert!(matches!(err, CodeTableError::InvalidWord { .. }), "{bad}");
    }
}

#[test]
fn rejects_empty_handler() {
    let err = load_err("[[m]]\ncode = \"M3\"\nhandler = \"  \"");
    assert!(matches!(err, CodeTableError::MissingHandler(c) if c == "M3"));
}
