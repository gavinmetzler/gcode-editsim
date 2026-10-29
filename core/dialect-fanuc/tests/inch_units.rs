//! G20 programs are converted to mm at the dialect boundary.

mod common;

use common::{axes, close, run, run_with, target};
use dialect_api::{ArcCenter, Command};
use dialect_fanuc::FanucOptions;
use ir::LengthUnit;

#[test]
fn axis_words_feed_and_radius_are_converted() {
    let r = run("G20\nG1 X1. Y-.5 F10.\nG2 X2. Y0 R1.\n");
    assert!(r.commands.contains(&Command::SetUnits(LengthUnit::Inch)));
    assert!(r.commands.contains(&Command::SetFeedRate(254.0)));
    let moves = r.moves();
    close(target(moves[0])[0].value, 25.4);
    close(target(moves[0])[1].value, -12.7);
    let Command::Arc {
        center: ArcCenter::Radius(radius),
        ..
    } = moves[1]
    else {
        panic!("expected R arc");
    };
    close(*radius, 25.4);
}

#[test]
fn arc_offsets_are_converted() {
    let r = run("G20 G3 X0 Y2. I-1. J1. F5.\n");
    let Command::Arc {
        center: ArcCenter::Offset(offsets),
        ..
    } = r.moves()[0]
    else {
        panic!("expected offset arc");
    };
    close(offsets[0].value, -25.4);
    close(offsets[1].value, 25.4);
}

#[test]
fn g21_switches_back() {
    let r = run("G20\nG0 X1.\nG21\nX1.\n");
    let moves = r.moves();
    close(target(moves[0])[0].value, 25.4);
    close(target(moves[1])[0].value, 1.0);
}

#[test]
fn units_change_applies_to_its_own_block() {
    let r = run("G20 G0 X1.\n");
    close(target(r.moves()[0])[0].value, 25.4);
}

#[test]
fn rotary_axes_are_never_converted() {
    let machine = axes(&[('X', Some('I'), false), ('A', None, true)]);
    let r = run_with(FanucOptions::default(), &machine, "G20 G0 X1. A90.\n");
    let t = target(r.moves()[0]);
    close(t[0].value, 25.4);
    close(t[1].value, 90.0);
}
