//! G95 selects feed per revolution: F is then mm (or inch) per spindle
//! revolution. The dialect passes the mode and the value on; the
//! interpreter turns them into mm/min with the spindle speed.

mod common;

use common::run;
use dialect_api::{Command, FeedMode};

#[test]
fn g95_selects_feed_per_revolution() {
    let r = run("G95\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    assert_eq!(
        r.commands,
        vec![Command::SetFeedMode(FeedMode::PerRevolution)]
    );
}

#[test]
fn a_feed_per_revolution_move_is_drawn_without_complaint() {
    let r = run("G95 G1 X1. F.1\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    assert!(r.commands.contains(&Command::SetFeedRate(0.1)));
    assert_eq!(r.moves().len(), 1);
}

#[test]
fn inch_feed_per_revolution_is_converted_to_mm() {
    let r = run("G20 G95 G1 X1. F.01\n");
    let feed = r.commands.iter().find_map(|c| match c {
        Command::SetFeedRate(f) => Some(*f),
        _ => None,
    });
    assert!(feed.is_some_and(|f| (f - 0.254).abs() < 1e-9), "{feed:?}");
}

#[test]
fn g94_returns_to_feed_per_minute() {
    let r = run("G95\nG94\n");
    assert_eq!(
        r.commands,
        vec![
            Command::SetFeedMode(FeedMode::PerRevolution),
            Command::SetFeedMode(FeedMode::PerMinute)
        ]
    );
}
