//! Property tests: the parser never panics and its output is well formed.

use gcode_syntax::{parse, parse_str, BinOp, Expr, Number, Program, Span};
use proptest::prelude::*;

/// Checks structural invariants that must hold for any input.
fn check_invariants(src: &[u8], program: &Program) -> Result<(), TestCaseError> {
    let line_lengths: Vec<usize> = src
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l).len())
        .collect();

    let mut previous_line = 0;
    for block in &program.blocks {
        prop_assert!(block.line > previous_line, "lines must increase");
        previous_line = block.line;

        let len = line_lengths
            .get(block.line as usize - 1)
            .copied()
            .ok_or_else(|| TestCaseError::fail("block line beyond end of input"))?;
        let in_line = |s: Span| s.start <= s.end && s.end <= len;

        prop_assert!(block.errors.len() <= 1, "at most one error per line");
        for w in &block.words {
            prop_assert!(w.letter.is_ascii_uppercase());
            prop_assert!(in_line(w.span));
        }
        for a in &block.assignments {
            prop_assert!(matches!(a.target, Expr::Var(_)));
            prop_assert!(in_line(a.span));
        }
        for c in &block.comments {
            prop_assert!(in_line(c.span));
        }
        for e in &block.errors {
            prop_assert!(in_line(e.span));
        }
        if let Some(level) = block.block_skip {
            prop_assert!((1..=9).contains(&level));
        }
    }
    Ok(())
}

/// Text built mostly from characters that mean something to the parser, so
/// generated cases reach deep into the grammar instead of failing on the
/// first byte.
fn gcode_like() -> impl Strategy<Value = String> {
    proptest::string::string_regex(
        r"[GMXYZIJKFSTNOPgxyz0-9 .+\-*/#=\[\]();%\t\r\nANDEQORSINmod]{0,300}",
    )
    .expect("valid regex")
}

fn number() -> impl Strategy<Value = Number> {
    prop_oneof![
        (-100_000i64..100_000).prop_map(|v| Number {
            value: v as f64,
            decimal_point: false,
        }),
        (-100_000i64..100_000, 0u32..4).prop_map(|(v, places)| Number {
            value: v as f64 / 10f64.powi(places as i32),
            decimal_point: true,
        }),
    ]
}

fn expr() -> impl Strategy<Value = Expr> {
    let leaf = number().prop_map(Expr::Number);
    leaf.prop_recursive(6, 48, 4, |inner| {
        let ops = prop::sample::select(vec![
            BinOp::Add,
            BinOp::Sub,
            BinOp::Mul,
            BinOp::Div,
            BinOp::Mod,
            BinOp::And,
            BinOp::Or,
            BinOp::Xor,
            BinOp::Eq,
            BinOp::Ne,
            BinOp::Gt,
            BinOp::Ge,
            BinOp::Lt,
            BinOp::Le,
        ]);
        prop_oneof![
            inner.clone().prop_map(|e| Expr::Var(Box::new(e))),
            // The parser folds `-literal` into the literal, so only negate
            // non-literals here.
            inner.clone().prop_map(|e| match e {
                Expr::Number(_) => Expr::Neg(Box::new(Expr::Var(Box::new(e)))),
                other => Expr::Neg(Box::new(other)),
            }),
            (ops, inner.clone(), inner.clone()).prop_map(|(op, l, r)| Expr::Binary {
                op,
                lhs: Box::new(l),
                rhs: Box::new(r),
            }),
            ("[A-Z]{1,5}", prop::collection::vec(inner, 1..4))
                .prop_map(|(name, args)| Expr::Call { name, args }),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn never_panics_on_arbitrary_bytes(src in prop::collection::vec(any::<u8>(), 0..2000)) {
        let program = parse(&src);
        check_invariants(&src, &program)?;
    }

    #[test]
    fn never_panics_on_arbitrary_text(src in any::<String>()) {
        let program = parse_str(&src);
        check_invariants(src.as_bytes(), &program)?;
    }

    #[test]
    fn never_panics_on_gcode_like_text(src in gcode_like()) {
        let program = parse_str(&src);
        check_invariants(src.as_bytes(), &program)?;
    }

    /// Well-formed words written in any of the usual styles parse back to
    /// exactly the letters and values that were generated.
    #[test]
    fn generated_words_round_trip(
        words in prop::collection::vec(("[A-MO-Z]", number()), 1..20),
        lowercase in any::<bool>(),
        spaced in any::<bool>(),
    ) {
        let sep = if spaced { " " } else { "" };
        let src: Vec<String> = words
            .iter()
            .map(|(letter, n)| {
                let letter = if lowercase { letter.to_lowercase() } else { letter.clone() };
                format!("{letter}{n}")
            })
            .collect();
        let src = src.join(sep);

        let program = parse_str(&src);
        prop_assert_eq!(program.blocks.len(), 1);
        let block = &program.blocks[0];
        prop_assert!(block.errors.is_empty(), "{:?} in {:?}", block.errors, src);
        let parsed: Vec<(String, Expr)> = block
            .words
            .iter()
            .map(|w| (w.letter.to_string(), w.value.clone()))
            .collect();
        let expected: Vec<(String, Expr)> = words
            .into_iter()
            .map(|(l, n)| (l, Expr::Number(n)))
            .collect();
        prop_assert_eq!(parsed, expected);
    }

    /// Displaying an expression and parsing it again gives the same tree.
    #[test]
    fn expression_display_round_trips(e in expr()) {
        let src = format!("X[{e}]");
        let program = parse_str(&src);
        let block = &program.blocks[0];
        prop_assert!(block.errors.is_empty(), "{:?} in {:?}", block.errors, src);
        prop_assert_eq!(&block.words[0].value, &e);
    }
}
