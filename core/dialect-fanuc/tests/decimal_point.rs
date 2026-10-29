//! Numbers without a decimal point use the least input increment by
//! default (Fanuc parameter DPI = 0), or are read as-is with
//! calculator-type input.

mod common;

use common::{axes, close, run, run_with, target, xyz};
use dialect_api::{ArcCenter, Command};
use dialect_fanuc::{DecimalInput, FanucOptions};
use ir::DiagCode;

#[test]
fn default_reads_whole_numbers_as_thousandths_of_a_mm() {
    let r = run("G0 X10 Y2.5 Z-1000\n");
    let t = target(r.moves()[0]);
    close(t[0].value, 0.01);
    close(t[1].value, 2.5);
    close(t[2].value, -1.0);
}

#[test]
fn warns_once_per_program() {
    let r = run("G0 X10\nY20\nZ30\n");
    let warnings: Vec<_> = r
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::ControllerDependent)
        .collect();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].dialect_specific);
    assert_eq!(warnings[0].src.line, 1);
    assert!(warnings[0].message.contains("DPI"));
}

#[test]
fn zero_needs_no_warning() {
    let r = run("G0 X0 Y0\n");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
}

#[test]
fn inch_increment_is_a_ten_thousandth() {
    let r = run("G20 G0 X10000\n");
    close(target(r.moves()[0])[0].value, 25.4);
}

#[test]
fn applies_to_arc_radius_and_offsets() {
    let r = run("G2 X1. Y0 R500 F100.\nG2 X1. Y0 I500 F100.\n");
    let moves = r.moves();
    let Command::Arc {
        center: ArcCenter::Radius(radius),
        ..
    } = moves[0]
    else {
        panic!()
    };
    close(*radius, 0.5);
    let Command::Arc {
        center: ArcCenter::Offset(o),
        ..
    } = moves[1]
    else {
        panic!()
    };
    close(o[0].value, 0.5);
}

#[test]
fn feed_spindle_and_tool_are_not_scaled() {
    let r = run("F100 S1200 T3\n");
    assert!(r.commands.contains(&Command::SetFeedRate(100.0)));
    assert!(r.commands.contains(&Command::SetSpindleSpeed {
        spindle: 0,
        rpm: 1200.0
    }));
    assert!(r.commands.contains(&Command::SelectTool(3)));
    assert!(r.diags.is_empty());
}

#[test]
fn rotary_increment_is_a_thousandth_of_a_degree() {
    let machine = axes(&[('X', Some('I'), false), ('B', None, true)]);
    let r = run_with(FanucOptions::default(), &machine, "G0 B90000\n");
    close(target(r.moves()[0])[0].value, 90.0);
}

#[test]
fn calculator_input_reads_numbers_as_written() {
    let options = FanucOptions {
        decimal_input: DecimalInput::Calculator,
    };
    let r = run_with(options, &xyz(), "G0 X10 Y2.5\n");
    let t = target(r.moves()[0]);
    close(t[0].value, 10.0);
    close(t[1].value, 2.5);
    assert!(r.diags.is_empty());
}
