//! Tests for `CodeKey`.

use dialect_api::CodeKey;

#[test]
fn equivalent_spellings_are_the_same_key() {
    let g1 = CodeKey::parse("G1").unwrap();
    assert_eq!(CodeKey::parse("G01"), Some(g1));
    assert_eq!(CodeKey::parse("g001"), Some(g1));
    assert_eq!(CodeKey::parse("G1.0"), Some(g1));
    assert_eq!(CodeKey::from_value('G', 1.0), Some(g1));
    assert_eq!(CodeKey::from_value('g', 1.0), Some(g1));
}

#[test]
fn decimal_codes() {
    let k = CodeKey::parse("G54.1").unwrap();
    assert_eq!(k.letter(), 'G');
    assert!((k.value() - 54.1).abs() < 1e-12);
    assert_eq!(k.to_string(), "G54.1");
    assert_ne!(k, CodeKey::parse("G54").unwrap());
    // Float noise from parsing is tolerated.
    assert_eq!(CodeKey::from_value('G', 54.1000000001), Some(k));
}

#[test]
fn display_is_normalised() {
    assert_eq!(CodeKey::parse("M03").unwrap().to_string(), "M3");
    assert_eq!(CodeKey::parse("G00").unwrap().to_string(), "G0");
    assert_eq!(CodeKey::parse("M30").unwrap().to_string(), "M30");
}

#[test]
fn invalid_codes() {
    for text in [
        "", "G", "1", "G-1", "G1.2.3", "G.", "GX", "G 1", "Ø1", "G1e3",
    ] {
        assert_eq!(CodeKey::parse(text), None, "{text:?}");
    }
    assert_eq!(CodeKey::from_value('G', 1.25), None);
    assert_eq!(CodeKey::from_value('G', -1.0), None);
    assert_eq!(CodeKey::from_value('G', f64::NAN), None);
    assert_eq!(CodeKey::from_value('G', 1e12), None);
    assert_eq!(CodeKey::from_value('#', 1.0), None);
}

#[test]
fn ordering_groups_by_letter_then_number() {
    let mut keys: Vec<CodeKey> = ["M3", "G54.1", "G2", "G54", "G10"]
        .iter()
        .map(|t| CodeKey::parse(t).unwrap())
        .collect();
    keys.sort();
    let shown: Vec<String> = keys.iter().map(ToString::to_string).collect();
    assert_eq!(shown, ["G2", "G10", "G54", "G54.1", "M3"]);
}
