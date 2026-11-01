//! Writing a value as Fanuc reads it (M4.4): the decimal-point rule, the
//! least input increment, inches, and the program's own number style.

use dialect_api::{Dialect, ValueKind};
use dialect_fanuc::{DecimalInput, Fanuc, FanucOptions};
use gcode_syntax::{Expr, Number, NumberStyle};
use ir::LengthUnit;
use proptest::prelude::*;

fn fanuc() -> Fanuc {
    Fanuc::new().expect("dialect")
}

fn calculator() -> Fanuc {
    Fanuc::with_options(FanucOptions {
        decimal_input: DecimalInput::Calculator,
    })
    .expect("dialect")
}

fn style(text: &str) -> NumberStyle {
    NumberStyle::of(text.as_bytes()).expect("literal")
}

/// `value` (mm or degrees) written in the style of `like`.
fn write(d: &Fanuc, value: f64, kind: ValueKind, units: LengthUnit, like: &str) -> String {
    d.format_value(value, kind, units, &style(like))
        .unwrap_or_else(|| panic!("{value} in the style of {like:?}"))
}

/// The number in `text` as Fanuc reads it, in mm or degrees.
fn read(d: &Fanuc, text: &str, kind: ValueKind, units: LengthUnit) -> f64 {
    let program = gcode_syntax::parse_str(&format!("X{text}"));
    let Expr::Number(n) = program.blocks[0].words[0].value.clone() else {
        panic!("{text:?} is not a number");
    };
    d.read_value(&n, kind, units).expect("Fanuc reads numbers")
}

const MM: LengthUnit = LengthUnit::Metric;
const INCH: LengthUnit = LengthUnit::Inch;
const LEN: ValueKind = ValueKind::Length;

#[test]
fn reads_numbers_as_the_interpreter_does() {
    let d = fanuc();
    assert_eq!(read(&d, "10.", LEN, MM), 10.0);
    assert_eq!(read(&d, "10", LEN, MM), 0.01, "DPI = 0: least increments");
    assert!((read(&d, "10000", LEN, INCH) - 25.4).abs() < 1e-12);
    assert!((read(&d, "1.", LEN, INCH) - 25.4).abs() < 1e-12);
    assert_eq!(
        read(&d, "90", ValueKind::Angle, INCH),
        0.09,
        "angles never in inches"
    );
    assert_eq!(read(&calculator(), "10", LEN, MM), 10.0);
}

#[test]
fn a_value_with_a_point_keeps_one() {
    let d = fanuc();
    assert_eq!(write(&d, 12.5, LEN, MM, "10."), "12.5");
    assert_eq!(write(&d, 12.0, LEN, MM, "10."), "12.");
    assert_eq!(write(&d, 12.0, LEN, MM, "10.000"), "12.000");
}

#[test]
fn rounds_to_the_least_increment() {
    let d = fanuc();
    // 3 decimals in mm, 4 in inches.
    assert_eq!(write(&d, 12.34567, LEN, MM, "1."), "12.346");
    assert_eq!(write(&d, 25.4 * 1.234567, LEN, INCH, "1."), "1.2346");
    assert_eq!(write(&d, 45.12345, ValueKind::Angle, INCH, "1."), "45.123");
}

#[test]
fn a_value_without_a_point_is_written_in_least_increments() {
    let d = fanuc();
    // X10 is 0.010 mm; 12.5 mm is 12500 increments.
    assert_eq!(write(&d, 12.5, LEN, MM, "10"), "12500");
    assert_eq!(write(&d, 25.4, LEN, INCH, "10"), "10000");
    assert_eq!(write(&d, 0.0004, LEN, MM, "10"), "0");
    assert_eq!(write(&d, -0.0126, LEN, MM, "10"), "-13");
}

#[test]
fn calculator_input_writes_whole_units_without_a_point() {
    let d = calculator();
    assert_eq!(write(&d, 12.0, LEN, MM, "10"), "12");
    // A value that is not whole needs a point, or it would mean 12 mm.
    assert_eq!(write(&d, 12.5, LEN, MM, "10"), "12.5");
    assert_eq!(write(&d, 12.0004, LEN, MM, "10"), "12");
}

#[test]
fn keeps_the_program_style() {
    let d = fanuc();
    assert_eq!(write(&d, 5.5, LEN, MM, "+010.0"), "+005.5");
    assert_eq!(write(&d, -0.25, LEN, MM, ".5"), "-.25");
    assert_eq!(write(&d, 25.0, LEN, MM, "0010"), "25000");
}

#[test]
fn refuses_what_it_cannot_write() {
    let d = fanuc();
    assert_eq!(d.format_value(f64::NAN, LEN, MM, &style("1.")), None);
    assert_eq!(d.format_value(f64::INFINITY, LEN, MM, &style("1")), None);
}

#[test]
fn other_dialects_do_not_rewrite_unless_they_say_so() {
    let osai = dialect_osai::Osai::new().expect("dialect");
    let n = Number {
        value: 1.0,
        decimal_point: true,
    };
    assert_eq!(osai.format_value(1.0, LEN, MM, &style("1.")), None);
    assert_eq!(osai.read_value(&n, LEN, MM), None);
}

/// Every option and unit the tests below try.
fn setups() -> impl Strategy<Value = (bool, LengthUnit, ValueKind)> {
    (
        any::<bool>(),
        prop_oneof![Just(MM), Just(INCH)],
        prop_oneof![Just(LEN), Just(ValueKind::Angle)],
    )
}

/// Half of the least input increment, in mm or degrees.
fn half_increment(units: LengthUnit, kind: ValueKind) -> f64 {
    match (kind, units) {
        (ValueKind::Angle, _) | (ValueKind::Length, LengthUnit::Metric) => 0.0005,
        (ValueKind::Length, LengthUnit::Inch) => 0.00005 * 25.4,
    }
}

proptest! {
    /// A written value reads back, through Fanuc's own rules, within half
    /// a least increment of what was asked for.
    #[test]
    fn written_values_read_back_within_half_an_increment(
        (calc, units, kind) in setups(),
        value in -9999.0f64..9999.0,
        like in prop_oneof![Just("1."), Just("1"), Just("-0.50"), Just("+010"), Just(".5"), Just("1.23456")],
    ) {
        let d = if calc { calculator() } else { fanuc() };
        let text = write(&d, value, kind, units, like);
        let back = read(&d, &text, kind, units);
        prop_assert!(
            (back - value).abs() <= half_increment(units, kind) + 1e-9,
            "{} written as {:?} reads {}", value, text, back
        );
    }

    /// A literal written again with its own value gives back the same text.
    #[test]
    fn a_literal_rewritten_with_its_own_value_is_unchanged(
        (calc, units, kind) in setups(),
        sign in prop_oneof![Just(""), Just("-"), Just("+")],
        int in "[1-9][0-9]{0,4}",
        frac in proptest::option::of("[0-9]{0,4}"),
    ) {
        let d = if calc { calculator() } else { fanuc() };
        let text = match &frac {
            Some(f) => format!("{sign}{int}.{f}"),
            None => format!("{sign}{int}"),
        };
        let value = read(&d, &text, kind, units);
        prop_assert_eq!(write(&d, value, kind, units, &text), text);
    }
}
