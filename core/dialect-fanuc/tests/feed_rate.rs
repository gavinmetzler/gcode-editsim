//! F is modal; feed moves need a feed rate.

mod common;

use common::{run, w};
use dialect_api::{Command, FeedMode};
use ir::{DiagCode, Severity};

#[test]
fn feed_carries_over() {
    let r = run("G1 X1. F250.\nX2.\n");
    assert!(r.diags.is_empty());
    assert_eq!(
        r.commands
            .iter()
            .filter(|c| matches!(c, Command::SetFeedRate(_)))
            .count(),
        1
    );
    assert_eq!(r.moves().len(), 2);
}

#[test]
fn feed_comes_before_the_move_it_applies_to() {
    let r = run("G1 X1. F250.\n");
    assert_eq!(
        r.commands,
        vec![
            Command::SetFeedRate(250.0),
            Command::Linear {
                target: vec![w(0, 1.0)]
            }
        ]
    );
}

#[test]
fn feed_move_without_feed_rate_is_an_error() {
    let r = run("G1 X1.\nG2 X1. I1.\nG0 X5.\n");
    assert_eq!(
        r.codes(),
        vec![DiagCode::MissingFeedRate, DiagCode::MissingFeedRate]
    );
    assert_eq!(
        r.moves(),
        vec![&Command::Rapid {
            target: vec![w(0, 5.0)]
        }]
    );
}

#[test]
fn zero_or_negative_feed_skips_the_whole_block() {
    let r = run("G91 G1 X1. F0\n");
    assert_eq!(r.codes(), vec![DiagCode::InvalidValue]);
    assert_eq!(r.diags[0].severity, Severity::Error);
    // Nothing from the block took effect, not even G91.
    assert!(r.commands.is_empty());
}

#[test]
fn g94_selects_feed_per_minute() {
    let r = run("G94\n");
    assert_eq!(r.commands, vec![Command::SetFeedMode(FeedMode::PerMinute)]);
}
