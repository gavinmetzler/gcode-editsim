//! How a number was written (M4.4): read from a word's text, and used to
//! write a new value the same way.

use gcode_syntax::{NumberStyle, Span};
use proptest::prelude::*;

fn style(text: &str) -> NumberStyle {
    NumberStyle::of(text.as_bytes()).unwrap_or_else(|| panic!("{text:?} is a literal"))
}

#[test]
fn reads_the_parts_of_a_literal() {
    let s = style("-012.50");
    assert!(s.negative && !s.plus);
    assert_eq!(s.int_digits, 3);
    assert!(s.decimal_point);
    assert_eq!(s.decimals, 2);

    let s = style("+.5");
    assert!(s.plus && !s.negative);
    assert_eq!(s.int_digits, 0);
    assert_eq!(s.decimals, 1);

    assert!(style("-012.50").zero_padded);
    assert!(!style("0.5").zero_padded && !style("10").zero_padded);

    let s = style("10");
    assert!(!s.decimal_point);
    assert_eq!((s.int_digits, s.decimals), (2, 0));

    let s = style("10.");
    assert!(s.decimal_point);
    assert_eq!((s.int_digits, s.decimals), (2, 0));

    // Spaces between the sign and the digits, as the parser allows.
    assert_eq!(style("- \t5.").sign_gap, " \t");
}

#[test]
fn refuses_what_is_not_a_plain_literal() {
    for bad in [
        "", "-", ".", "#1", "[1+2]", "1.2.3", "1e5", "--1", "1 2", "5-", " 5",
    ] {
        assert_eq!(NumberStyle::of(bad.as_bytes()), None, "{bad:?}");
    }
}

#[test]
fn writes_decimals_as_written_or_as_needed_up_to_the_limit() {
    let s = style("10.");
    assert_eq!(s.render_decimal(10.0, 3).as_deref(), Some("10."));
    assert_eq!(s.render_decimal(12.25, 3).as_deref(), Some("12.25"));
    assert_eq!(s.render_decimal(12.2346, 3).as_deref(), Some("12.235"));
    let s = style("10.500");
    assert_eq!(s.render_decimal(12.25, 3).as_deref(), Some("12.250"));
    // More decimals than the limit were written: never fewer than that.
    let s = style("1.23456");
    assert_eq!(s.render_decimal(2.0, 3).as_deref(), Some("2.00000"));
}

#[test]
fn keeps_leading_zeros_and_signs() {
    assert_eq!(
        style("010.").render_decimal(5.5, 3).as_deref(),
        Some("005.5")
    );
    assert_eq!(
        style("10.").render_decimal(5.5, 3).as_deref(),
        Some("5.5"),
        "not padded"
    );
    assert_eq!(style("10").render_integer(0.0).as_deref(), Some("0"));
    assert_eq!(style(".5").render_decimal(0.25, 3).as_deref(), Some(".25"));
    assert_eq!(
        style(".5").render_decimal(-0.25, 3).as_deref(),
        Some("-.25")
    );
    assert_eq!(style(".5").render_decimal(0.0, 3).as_deref(), Some(".0"));
    assert_eq!(style("+1.").render_decimal(2.0, 3).as_deref(), Some("+2."));
    assert_eq!(style("+1.").render_decimal(-2.0, 3).as_deref(), Some("-2."));
    assert_eq!(
        style("- 1.").render_decimal(-2.0, 3).as_deref(),
        Some("- 2.")
    );
    // The gap goes with the sign: no sign, no gap.
    assert_eq!(style("- 1.").render_decimal(2.0, 3).as_deref(), Some("2."));
}

#[test]
fn a_value_that_rounds_to_zero_has_no_minus_sign() {
    assert_eq!(
        style("1.").render_decimal(-0.0001, 3).as_deref(),
        Some("0.")
    );
    assert_eq!(style("1").render_integer(-0.0).as_deref(), Some("0"));
}

#[test]
fn writes_whole_numbers_without_a_point() {
    assert_eq!(style("10").render_integer(2500.0).as_deref(), Some("2500"));
    assert_eq!(style("0010").render_integer(25.0).as_deref(), Some("0025"));
    assert_eq!(style("+10").render_integer(-25.0).as_deref(), Some("-25"));
    assert_eq!(style("10").render_integer(2.5), None, "not whole");
    assert_eq!(style("10").render_integer(f64::NAN), None);
    assert_eq!(style("10.").render_decimal(f64::INFINITY, 3), None);
}

#[test]
fn finds_the_value_text_of_a_word() {
    let line = b"N10 G1 X - 10.5 Y.5 Z#1";
    let program = gcode_syntax::parse(line);
    let words = &program.blocks[0].words;
    let text = |i: usize| {
        let span = words[i].value_span(line).expect("a literal");
        std::str::from_utf8(&line[span.start..span.end])
            .unwrap()
            .to_owned()
    };
    assert_eq!(text(1), "- 10.5");
    assert_eq!(text(2), ".5");
    assert_eq!(
        words[3].value_span(line),
        None,
        "a variable is not a literal"
    );
    assert_eq!(
        words[1].value_span(b"short"),
        None,
        "a line that is not the word's"
    );
    let _ = Span::default();
}

proptest! {
    /// Writing a literal's own value in its own style gives back the same
    /// text, for the ordinary literals CAM systems write.
    #[test]
    fn a_literal_written_in_its_own_style_is_unchanged(
        sign in prop_oneof![Just(""), Just("-"), Just("+")],
        int in "[0-9]{0,6}",
        frac in proptest::option::of("[0-9]{0,6}"),
    ) {
        prop_assume!(!int.is_empty() || frac.as_deref().is_some_and(|f| !f.is_empty()));
        let text = match &frac {
            Some(f) => format!("{sign}{int}.{f}"),
            None => format!("{sign}{int}"),
        };
        let s = style(&text);
        let value: f64 = text.trim_start_matches('+').parse().unwrap();
        // A zero written with a minus sign is written without one.
        prop_assume!(!(sign == "-" && value == 0.0));
        let written = match &frac {
            Some(f) => s.render_decimal(value, f.len().max(3)),
            None => s.render_integer(value),
        };
        prop_assert_eq!(written.as_deref(), Some(text.as_str()));
    }

    /// Whatever is written reads back as a number within half of the last
    /// decimal written.
    #[test]
    fn what_is_written_reads_back_close(
        value in -1.0e6f64..1.0e6,
        decimals in 0usize..6,
        max in 0usize..6,
    ) {
        let template = format!("1.{}", "0".repeat(decimals));
        let written = style(&template).render_decimal(value, max).unwrap();
        let back: f64 = written.trim_start_matches('+').parse().unwrap();
        let places = decimals.max(max) as i32;
        prop_assert!((back - value).abs() <= 0.5 * 10f64.powi(-places) + 1e-9, "{} -> {}", value, written);
        prop_assert!(NumberStyle::of(written.as_bytes()).is_some());
    }
}
