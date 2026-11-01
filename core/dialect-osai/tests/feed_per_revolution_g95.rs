//! G95 selects feed per revolution and G94 feed per minute (manual p. 1-7,
//! 1-12). Under G95, F is mm (or inch) per spindle revolution; the
//! interpreter turns it into mm/min with the spindle speed.

mod common;

use common::run;
use dialect_api::{Command, FeedMode};

#[test]
fn g95_selects_feed_per_revolution() {
    let r = run("G71 G95 G1 X1 F.1\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    assert!(r
        .commands
        .contains(&Command::SetFeedMode(FeedMode::PerRevolution)));
    assert!(r.commands.contains(&Command::SetFeedRate(0.1)));
    assert_eq!(r.moves().len(), 1);
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
