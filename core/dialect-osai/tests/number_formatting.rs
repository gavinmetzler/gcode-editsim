//! Writing a value as the 10 Series reads it (M4.9a): numbers are read as
//! written (`X10` is 10 mm, with or without a point; manual p. 2-4), up to
//! 9 decimals (p. 1-15, "Programming restrictions for long real formats").

use dialect_api::{Dialect, ValueKind};
use dialect_osai::Osai;
use gcode_syntax::{Expr, NumberStyle};
use ir::LengthUnit;
use proptest::prelude::*;

const MM: LengthUnit = LengthUnit::Metric;
const INCH: LengthUnit = LengthUnit::Inch;
const LEN: ValueKind = ValueKind::Length;

fn osai() -> Osai {
    Osai::new().expect("dialect")
}

fn write(value: f64, kind: ValueKind, units: LengthUnit, like: &str) -> String {
    let style = NumberStyle::of(like.as_bytes()).expect("literal");
    osai()
        .format_value(value, kind, units, &style)
        .unwrap_or_else(|| panic!("{value} like {like:?}"))
}

fn read(text: &str, kind: ValueKind, units: LengthUnit) -> f64 {
    let program = gcode_syntax::parse_str(&format!("X{text}"));
    let Expr::Number(n) = program.blocks[0].words[0].value.clone() else {
        panic!("{text:?} is not a number");
    };
    osai()
        .read_value(&n, kind, units)
        .expect("Osai reads numbers")
}

#[test]
fn numbers_are_read_as_written() {
    assert_eq!(
        read("10", LEN, MM),
        10.0,
        "no point is still mm, unlike Fanuc"
    );
    assert_eq!(read("10.5", LEN, MM), 10.5);
    assert!((read("1", LEN, INCH) - 25.4).abs() < 1e-12);
    assert_eq!(read("90", ValueKind::Angle, INCH), 90.0);
}

#[test]
fn written_values_keep_the_programs_decimals_and_add_what_they_need() {
    assert_eq!(write(12.5, LEN, MM, "1.5"), "12.5");
    assert_eq!(
        write(12.0, LEN, MM, "1.5"),
        "12.0",
        "the decimal written is kept"
    );
    assert_eq!(write(12.3456, LEN, MM, "1.5"), "12.346", "up to 0.001 mm");
    assert_eq!(
        write(25.4 * 1.23456, LEN, INCH, "1.5"),
        "1.2346",
        "up to 0.0001 inch"
    );
    assert_eq!(write(45.12345, ValueKind::Angle, MM, "1.5"), "45.123");
}

#[test]
fn a_number_without_a_point_gets_one_only_when_it_must() {
    assert_eq!(write(12.0, LEN, MM, "10"), "12");
    assert_eq!(write(12.5, LEN, MM, "10"), "12.5", "12 would be 12 mm");
    assert_eq!(write(12.0004, LEN, MM, "10"), "12");
    assert_eq!(write(-0.0004, LEN, MM, "10"), "0");
}

#[test]
fn refuses_what_it_cannot_write() {
    let style = NumberStyle::of(b"1.").expect("literal");
    assert_eq!(osai().format_value(f64::NAN, LEN, MM, &style), None);
}

#[test]
fn arcs_name_their_radius_r_and_their_centre_i_and_j_in_any_plane() {
    let d = osai();
    assert_eq!(d.arc_radius_letter(), Some('R'));
    let plane = ir::ArcPlane {
        first: 2,
        second: 0,
    };
    assert_eq!(
        d.arc_centre_letters(plane, [Some('K'), Some('I')]),
        [Some('I'), Some('J')]
    );
}

proptest! {
    /// A value written reads back within half of 0.001 mm (0.0001 inch).
    #[test]
    fn written_values_read_back_close(
        value in -9999.0f64..9999.0,
        inch in any::<bool>(),
        like in prop_oneof![Just("1"), Just("1.5"), Just("-2.25"), Just("+3.125")],
    ) {
        let units = if inch { INCH } else { MM };
        let text = write(value, LEN, units, like);
        let half = if inch { 0.00005 * 25.4 } else { 0.0005 };
        prop_assert!((read(&text, LEN, units) - value).abs() <= half + 1e-9, "{} -> {}", value, text);
    }

    /// A literal written again with its own value gives back the same text.
    #[test]
    fn a_literal_rewritten_with_its_own_value_is_unchanged(
        inch in any::<bool>(),
        sign in prop_oneof![Just(""), Just("-")],
        int in "[1-9][0-9]{0,3}",
        frac in proptest::option::of("[0-9]{1,3}"),
    ) {
        let units = if inch { INCH } else { MM };
        let text = match &frac { Some(f) => format!("{sign}{int}.{f}"), None => format!("{sign}{int}") };
        let value = read(&text, LEN, units);
        prop_assert_eq!(write(value, LEN, units, &text), text);
    }
}
