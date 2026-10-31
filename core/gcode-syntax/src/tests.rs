//! Unit tests for the parser.

use crate::*;

fn one(src: &str) -> Block {
    let mut program = parse_str(src);
    assert_eq!(program.blocks.len(), 1, "expected one block for {src:?}");
    program.blocks.remove(0)
}

fn num(value: f64) -> Expr {
    Expr::number(value, false)
}

fn dec(value: f64) -> Expr {
    Expr::number(value, true)
}

fn var(n: f64) -> Expr {
    Expr::Var(Box::new(num(n)))
}

fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// The words of a block as `(letter, expr)` pairs, ignoring spans.
fn words(block: &Block) -> Vec<(char, Expr)> {
    block
        .words
        .iter()
        .map(|w| (w.letter, w.value.clone()))
        .collect()
}

fn error_kinds(block: &Block) -> Vec<SyntaxErrorKind> {
    block.errors.iter().map(|e| e.kind.clone()).collect()
}

// ---------------------------------------------------------------- words

#[test]
fn simple_words_with_spaces() {
    let b = one("G1 X10 Y20.5");
    assert_eq!(
        words(&b),
        vec![('G', num(1.0)), ('X', num(10.0)), ('Y', dec(20.5))]
    );
    assert!(b.errors.is_empty());
}

#[test]
fn no_spaces_between_words() {
    let b = one("G01X10.5Y-3Z.25F100");
    assert_eq!(
        words(&b),
        vec![
            ('G', num(1.0)),
            ('X', dec(10.5)),
            ('Y', num(-3.0)),
            ('Z', dec(0.25)),
            ('F', num(100.0)),
        ]
    );
}

#[test]
fn lowercase_letters_are_upper_cased() {
    let b = one("g1 x10 y-2");
    assert_eq!(
        words(&b),
        vec![('G', num(1.0)), ('X', num(10.0)), ('Y', num(-2.0))]
    );
}

#[test]
fn negative_and_explicit_positive_numbers() {
    let b = one("X-10 Y+5 Z-0.5");
    assert_eq!(
        words(&b),
        vec![('X', num(-10.0)), ('Y', num(5.0)), ('Z', dec(-0.5))]
    );
}

#[test]
fn decimal_point_without_leading_digit() {
    let b = one("X.5 Y-.25");
    assert_eq!(words(&b), vec![('X', dec(0.5)), ('Y', dec(-0.25))]);
}

#[test]
fn trailing_decimal_point_is_remembered() {
    let b = one("X10. Y10");
    assert_eq!(words(&b), vec![('X', dec(10.0)), ('Y', num(10.0))]);
}

#[test]
fn decimal_g_code_keeps_fraction() {
    let b = one("G54.1 P1");
    assert_eq!(words(&b), vec![('G', dec(54.1)), ('P', num(1.0))]);
}

#[test]
fn space_between_letter_and_value_is_allowed() {
    let b = one("X 10 Y - 2");
    assert_eq!(words(&b), vec![('X', num(10.0)), ('Y', num(-2.0))]);
}

#[test]
fn leading_zeros() {
    let b = one("G00 M0003");
    assert_eq!(words(&b), vec![('G', num(0.0)), ('M', num(3.0))]);
}

#[test]
fn word_spans_cover_the_word() {
    let b = one("  G1 X-10.5");
    let spans: Vec<Span> = b.words.iter().map(|w| w.span).collect();
    assert_eq!(spans, vec![Span::new(2, 4), Span::new(5, 11)]);
}

#[test]
fn letter_without_value_is_an_error() {
    let b = one("G1 X Y2");
    assert_eq!(words(&b), vec![('G', num(1.0))]);
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::MissingValue('X')]);
    assert_eq!(b.errors[0].span, Span::new(3, 4));
}

#[test]
fn letter_at_end_of_line_is_an_error() {
    let b = one("G1 X");
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::MissingValue('X')]);
}

#[test]
fn sign_without_number_is_an_error() {
    let b = one("X-");
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::ExpectedExpression]);
}

#[test]
fn lone_decimal_point_is_an_error() {
    let b = one("X.");
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::InvalidNumber]);
}

#[test]
fn two_decimal_points_stop_at_the_second() {
    // `X1.2.3` reads `X1.2`, then `.3` has no address letter.
    let b = one("X1.2.3");
    assert_eq!(words(&b), vec![('X', dec(1.2))]);
    assert_eq!(
        error_kinds(&b),
        vec![SyntaxErrorKind::UnexpectedCharacter('.')]
    );
}

#[test]
fn number_without_letter_is_an_error() {
    let b = one("G1 10");
    assert_eq!(
        error_kinds(&b),
        vec![SyntaxErrorKind::UnexpectedCharacter('1')]
    );
}

#[test]
fn huge_numbers_are_out_of_range() {
    let src = format!("X{}", "9".repeat(400));
    let b = one(&src);
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::NumberOutOfRange]);
}

#[test]
fn exponent_letter_is_a_separate_word() {
    // G-code has no exponent notation; `E` is an address letter.
    let b = one("X1E5");
    assert_eq!(words(&b), vec![('X', num(1.0)), ('E', num(5.0))]);
}

// ----------------------------------------------- sequence numbers and skip

#[test]
fn sequence_number_at_start() {
    let b = one("N10 G1");
    assert_eq!(b.sequence_number, Some(10));
    assert_eq!(words(&b), vec![('G', num(1.0))]);
}

#[test]
fn lowercase_sequence_number_without_space() {
    let b = one("n0020g0");
    assert_eq!(b.sequence_number, Some(20));
    assert_eq!(words(&b), vec![('G', num(0.0))]);
}

#[test]
fn n_word_later_in_block_stays_a_word() {
    let b = one("G1 N5");
    assert_eq!(b.sequence_number, None);
    assert_eq!(words(&b), vec![('G', num(1.0)), ('N', num(5.0))]);
}

#[test]
fn second_n_word_stays_a_word() {
    let b = one("N1 N2");
    assert_eq!(b.sequence_number, Some(1));
    assert_eq!(words(&b), vec![('N', num(2.0))]);
}

#[test]
fn invalid_sequence_numbers() {
    for src in ["N1.5", "N10.", "N-1", "N#1", "N99999999999"] {
        let b = one(src);
        assert_eq!(
            error_kinds(&b),
            vec![SyntaxErrorKind::InvalidSequenceNumber],
            "{src}"
        );
        assert_eq!(b.sequence_number, None, "{src}");
    }
}

#[test]
fn block_skip_plain() {
    let b = one("/N10 G0 X0");
    assert_eq!(b.block_skip, Some(1));
    assert_eq!(b.sequence_number, Some(10));
}

#[test]
fn block_skip_with_level_and_leading_space() {
    let b = one("  /3 G0");
    assert_eq!(b.block_skip, Some(3));
    assert_eq!(words(&b), vec![('G', num(0.0))]);
}

#[test]
fn block_skip_level_zero_is_an_error() {
    let b = one("/0 G0");
    assert_eq!(
        error_kinds(&b),
        vec![SyntaxErrorKind::InvalidBlockSkipLevel]
    );
}

#[test]
fn slash_mid_block_is_an_error() {
    let b = one("G0 /X1");
    assert_eq!(
        error_kinds(&b),
        vec![SyntaxErrorKind::UnexpectedCharacter('/')]
    );
}

#[test]
fn no_block_skip_by_default() {
    assert_eq!(one("G0").block_skip, None);
}

// ------------------------------------------------------------- comments

#[test]
fn paren_comment() {
    let b = one("G0 X1 (rapid to start) Y2");
    assert_eq!(b.comments.len(), 1);
    assert_eq!(b.comments[0].kind, CommentKind::Paren);
    assert_eq!(b.comments[0].text, "rapid to start");
    assert_eq!(b.comments[0].span, Span::new(6, 22));
    assert_eq!(words(&b).len(), 3);
}

#[test]
fn semicolon_comment_runs_to_end_of_line() {
    let b = one("G0 X1 ; move (not a paren comment) X99");
    assert_eq!(words(&b), vec![('G', num(0.0)), ('X', num(1.0))]);
    assert_eq!(b.comments.len(), 1);
    assert_eq!(b.comments[0].kind, CommentKind::Semicolon);
    assert_eq!(b.comments[0].text, " move (not a paren comment) X99");
}

#[test]
fn comment_only_line() {
    let b = one("(TOOL 1 - 10MM ENDMILL)");
    assert!(b.words.is_empty());
    assert_eq!(b.comments[0].text, "TOOL 1 - 10MM ENDMILL");
}

#[test]
fn several_comments_and_no_spaces() {
    let b = one("G1(a)X1(b);c");
    let texts: Vec<&str> = b.comments.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(texts, vec!["a", "b", "c"]);
    assert_eq!(words(&b).len(), 2);
}

#[test]
fn parentheses_do_not_nest() {
    // The first `)` closes the comment, so ` c)` is parsed as G-code.
    let b = one("(a (b) c)");
    assert_eq!(b.comments[0].text, "a (b");
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::MissingValue('C')]);
}

#[test]
fn unterminated_comment_keeps_text_and_reports_error() {
    let b = one("G1 X1 (no close");
    assert_eq!(words(&b), vec![('G', num(1.0)), ('X', num(1.0))]);
    assert_eq!(b.comments[0].text, "no close");
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::UnterminatedComment]);
    assert_eq!(b.errors[0].span, Span::new(6, 7));
}

#[test]
fn unterminated_comment_does_not_leak_to_next_line() {
    let p = parse_str("(open\nG1 X1\n");
    assert_eq!(p.blocks.len(), 2);
    assert_eq!(p.blocks[1].errors, vec![]);
    assert_eq!(words(&p.blocks[1]).len(), 2);
}

#[test]
fn comment_with_non_utf8_bytes_is_lossy() {
    let p = parse(b"(caf\xe9)");
    assert_eq!(p.blocks[0].comments[0].text, "caf\u{fffd}");
    assert!(p.blocks[0].errors.is_empty());
}

#[test]
fn comment_with_utf8_text() {
    let b = one("(Ø10 Fräser)");
    assert_eq!(b.comments[0].text, "Ø10 Fräser");
}

// ---------------------------------------------------------- expressions

#[test]
fn variable_as_word_value() {
    let b = one("X#100 Y-#1");
    assert_eq!(
        words(&b),
        vec![('X', var(100.0)), ('Y', Expr::Neg(Box::new(var(1.0))))]
    );
}

#[test]
fn bracketed_expression_as_word_value() {
    let b = one("X[#1+2]");
    assert_eq!(words(&b), vec![('X', bin(BinOp::Add, var(1.0), num(2.0)))]);
}

#[test]
fn operator_precedence() {
    // * binds tighter than +, comparisons bind loosest.
    let b = one("X[1+2*3 GT 4]");
    let expected = bin(
        BinOp::Gt,
        bin(BinOp::Add, num(1.0), bin(BinOp::Mul, num(2.0), num(3.0))),
        num(4.0),
    );
    assert_eq!(words(&b), vec![('X', expected)]);
}

#[test]
fn operators_are_left_associative() {
    let b = one("X[8-4-2]");
    let expected = bin(BinOp::Sub, bin(BinOp::Sub, num(8.0), num(4.0)), num(2.0));
    assert_eq!(words(&b), vec![('X', expected)]);
}

#[test]
fn keyword_operators_are_case_insensitive() {
    let b = one("X[#1 and 3 Or #2 mod 2]");
    let expected = bin(
        BinOp::Or,
        bin(BinOp::And, var(1.0), num(3.0)),
        bin(BinOp::Mod, var(2.0), num(2.0)),
    );
    assert_eq!(words(&b), vec![('X', expected)]);
}

#[test]
fn nested_brackets_and_unary_minus() {
    let b = one("Z-[[#1+1]*2]");
    let expected = Expr::Neg(Box::new(bin(
        BinOp::Mul,
        bin(BinOp::Add, var(1.0), num(1.0)),
        num(2.0),
    )));
    assert_eq!(words(&b), vec![('Z', expected)]);
}

#[test]
fn indirect_variables() {
    let b = one("X#[#1+1] Y##2");
    assert_eq!(
        words(&b),
        vec![
            (
                'X',
                Expr::Var(Box::new(bin(BinOp::Add, var(1.0), num(1.0))))
            ),
            ('Y', Expr::Var(Box::new(var(2.0)))),
        ]
    );
}

#[test]
fn function_calls() {
    let b = one("X[SIN[30]*#1] Y[atan[1, 2]]");
    assert_eq!(
        words(&b),
        vec![
            (
                'X',
                bin(
                    BinOp::Mul,
                    Expr::Call {
                        name: "SIN".into(),
                        args: vec![num(30.0)]
                    },
                    var(1.0)
                )
            ),
            (
                'Y',
                Expr::Call {
                    name: "ATAN".into(),
                    args: vec![num(1.0), num(2.0)]
                }
            ),
        ]
    );
}

#[test]
fn function_without_brackets_is_an_error() {
    let b = one("X[SIN 30]");
    assert_eq!(
        error_kinds(&b),
        vec![SyntaxErrorKind::ExpectedFunctionArguments("SIN".into())]
    );
}

#[test]
fn unclosed_bracket_is_an_error() {
    let b = one("G1 X[#1+2 Y3");
    assert_eq!(words(&b), vec![('G', num(1.0))]);
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::UnclosedBracket]);
    assert_eq!(b.errors[0].span, Span::new(4, 5));
}

#[test]
fn empty_brackets_are_an_error() {
    let b = one("X[]");
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::ExpectedExpression]);
}

#[test]
fn hash_without_number_is_an_error() {
    for src in ["X#", "X#A", "#=1"] {
        let b = one(src);
        assert_eq!(
            error_kinds(&b),
            vec![SyntaxErrorKind::MissingVariableNumber],
            "{src}"
        );
    }
}

#[test]
fn deep_nesting_is_rejected_not_overflowed() {
    let deep = format!("X{}1{}", "[".repeat(10_000), "]".repeat(10_000));
    let b = one(&deep);
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::NestingTooDeep]);

    let signs = format!("X[{}1]", "-".repeat(10_000));
    assert_eq!(
        error_kinds(&one(&signs)),
        vec![SyntaxErrorKind::NestingTooDeep]
    );

    let hashes = format!("X{}1", "#".repeat(10_000));
    assert_eq!(
        error_kinds(&one(&hashes)),
        vec![SyntaxErrorKind::NestingTooDeep]
    );
}

#[test]
fn nesting_up_to_the_limit_is_fine() {
    let n = 20;
    let src = format!("X{}1{}", "[".repeat(n), "]".repeat(n));
    let b = one(&src);
    assert!(b.errors.is_empty(), "{:?}", b.errors);
    assert_eq!(words(&b), vec![('X', num(1.0))]);
}

// ---------------------------------------------------------- assignments

#[test]
fn simple_assignment() {
    let b = one("#100=5");
    assert_eq!(b.assignments.len(), 1);
    assert_eq!(b.assignments[0].target, var(100.0));
    assert_eq!(b.assignments[0].value, num(5.0));
    assert_eq!(b.assignments[0].span, Span::new(0, 6));
}

#[test]
fn assignment_with_unbracketed_expression() {
    let b = one("#1 = #2 + 3 * SIN[#3]");
    assert_eq!(
        b.assignments[0].value,
        bin(
            BinOp::Add,
            var(2.0),
            bin(
                BinOp::Mul,
                num(3.0),
                Expr::Call {
                    name: "SIN".into(),
                    args: vec![var(3.0)]
                }
            )
        )
    );
}

#[test]
fn assignment_followed_by_words() {
    let b = one("#1=5 G1 X#1");
    assert_eq!(b.assignments[0].value, num(5.0));
    assert_eq!(words(&b), vec![('G', num(1.0)), ('X', var(1.0))]);
}

#[test]
fn assignment_without_equals_is_an_error() {
    let b = one("#1 5");
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::ExpectedEquals]);
}

#[test]
fn assignment_without_value_is_an_error() {
    let b = one("#1=");
    assert_eq!(error_kinds(&b), vec![SyntaxErrorKind::ExpectedExpression]);
}

#[test]
fn n_word_after_assignment_is_not_a_sequence_number() {
    let b = one("#1=2 N5");
    assert_eq!(b.sequence_number, None);
    assert_eq!(words(&b), vec![('N', num(5.0))]);
}

// ----------------------------------------------------- lines and blocks

#[test]
fn blank_lines_produce_no_blocks_but_count_for_line_numbers() {
    let p = parse_str("\n  \nG0\n\t\nG1\n");
    let lines: Vec<u32> = p.blocks.iter().map(|b| b.line).collect();
    assert_eq!(lines, vec![3, 5]);
}

#[test]
fn crlf_line_endings() {
    let p = parse_str("G0 X1\r\nG1 Y2\r\n");
    assert_eq!(p.blocks.len(), 2);
    assert!(p.blocks.iter().all(|b| b.errors.is_empty()));
    assert_eq!(p.blocks[1].line, 2);
}

#[test]
fn last_line_without_newline() {
    let p = parse_str("G0\nG1");
    assert_eq!(p.blocks.len(), 2);
}

#[test]
fn empty_input() {
    assert!(parse(b"").blocks.is_empty());
}

#[test]
fn percent_marks_program_delimiter() {
    let p = parse_str("%\nO1000\nM30\n%\n");
    assert!(p.blocks[0].program_delimiter);
    assert!(p.blocks[0].words.is_empty());
    assert!(!p.blocks[1].program_delimiter);
    assert_eq!(words(&p.blocks[1]), vec![('O', num(1000.0))]);
    assert!(p.blocks[3].program_delimiter);
}

#[test]
fn colon_program_number_reads_as_o() {
    // ISO-code programs may write the program number as ":0099" instead of
    // "O0099" (Fanuc; see docs/dialect-notes/fanuc.md).
    let p = parse_str(":0099\n :99 \nG0 X1.\n");
    assert!(p.blocks[0].errors.is_empty(), "{:?}", p.blocks[0].errors);
    assert_eq!(words(&p.blocks[0]), vec![('O', num(99.0))]);
    assert_eq!(words(&p.blocks[1]), vec![('O', num(99.0))]);
    assert_eq!(p.blocks[0].words[0].span, Span::new(0, 5));
}

#[test]
fn colon_anywhere_else_is_still_an_error() {
    for src in ["G0 :1\n", ":\n", "N10 :5\n"] {
        let p = parse_str(src);
        assert!(!p.blocks[0].errors.is_empty(), "{src:?}");
    }
}

#[test]
fn error_on_one_line_does_not_affect_the_next() {
    let p = parse_str("G1 X$ Y2\nG1 X3\n");
    assert_eq!(p.blocks.len(), 2);
    assert_eq!(
        error_kinds(&p.blocks[0]),
        vec![SyntaxErrorKind::MissingValue('X')]
    );
    assert!(p.blocks[1].errors.is_empty());
    assert_eq!(words(&p.blocks[1]), vec![('G', num(1.0)), ('X', num(3.0))]);
}

#[test]
fn parsing_stops_at_first_error_in_a_line() {
    let b = one("G1 @ X1 Y2");
    assert_eq!(words(&b), vec![('G', num(1.0))]);
    assert_eq!(b.errors.len(), 1);
    assert_eq!(b.errors[0].span, Span::new(3, 4));
}

#[test]
fn binary_garbage_reports_errors_without_panicking() {
    let p = parse(b"\x00\xff\xfe G1\n\x1b[31m\nG0 X1\n");
    assert_eq!(p.blocks.len(), 3);
    assert_eq!(
        error_kinds(&p.blocks[0]),
        vec![SyntaxErrorKind::UnexpectedCharacter('\0')]
    );
    assert!(!p.blocks[1].errors.is_empty());
    assert!(p.blocks[2].errors.is_empty());
}

#[test]
fn invalid_utf8_reports_replacement_character() {
    let b = &parse(b"G1 \xff").blocks[0];
    assert_eq!(
        error_kinds(b),
        vec![SyntaxErrorKind::UnexpectedCharacter('\u{fffd}')]
    );
    assert_eq!(b.errors[0].span, Span::new(3, 4));
}

#[test]
fn non_ascii_character_span_covers_whole_character() {
    let b = one("G1 é");
    assert_eq!(
        error_kinds(&b),
        vec![SyntaxErrorKind::UnexpectedCharacter('é')]
    );
    assert_eq!(b.errors[0].span, Span::new(3, 5));
}

#[test]
fn very_long_line() {
    let src = "G1 X1 ".repeat(200_000);
    let b = one(&src);
    assert!(b.errors.is_empty());
    assert_eq!(b.words.len(), 400_000);
}

#[test]
fn program_errors_iterator() {
    let p = parse_str("G1\nX\nG0\n(oops\n");
    let lines: Vec<u32> = p.errors().map(|(line, _)| line).collect();
    assert_eq!(lines, vec![2, 4]);
}

// -------------------------------------------------------------- display

#[test]
fn expressions_display_in_g_code_syntax() {
    let b = one("X[#1+2*SIN[30.]] Y#[#1] Z-#2 A10. B-0.5 C#5.");
    let shown: Vec<String> = b.words.iter().map(|w| w.value.to_string()).collect();
    assert_eq!(
        shown,
        vec![
            "[#1 + [2 * SIN[30.]]]",
            "#[#1]",
            "-#2",
            "10.",
            "-0.5",
            "#5."
        ]
    );
}

#[test]
fn error_messages_are_readable() {
    let b = one("G1 X");
    assert_eq!(b.errors[0].to_string(), "address X has no value");
}
