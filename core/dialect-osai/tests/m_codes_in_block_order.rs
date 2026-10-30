//! Standard M functions (manual chapter 6) and when they act within a
//! block: M3, M4, M6, M7, M8, M9 and M19 before the move; M0, M1, M2, M5
//! and M30 after it.

mod common;

use common::run;
use dialect_api::{Command, CoolantState, SpindleState};

fn kinds(r: &common::Run) -> Vec<String> {
    r.commands
        .iter()
        .filter_map(|c| {
            Some(match c {
                Command::Rapid { .. } => "move".to_owned(),
                Command::SetSpindleState { state, .. } => format!("spindle {state:?}"),
                Command::Coolant(s) => format!("coolant {s:?}"),
                Command::Stop { optional } => format!("stop {optional}"),
                Command::ProgramEnd { rewind } => format!("end {rewind}"),
                Command::ToolChange { tool } => format!("tool {tool}"),
                _ => return None,
            })
        })
        .collect()
}

#[test]
fn m9_acts_before_the_move_unlike_fanuc() {
    let r = run("G71 X1 M9\n");
    assert_eq!(kinds(&r), vec!["coolant Off", "move"]);
}

#[test]
fn m5_and_stops_act_after_the_move() {
    let r = run("G71 X1 M5\nX2 M0\nX3 M1\nX4 M30\n");
    assert_eq!(
        kinds(&r),
        vec![
            "move",
            "spindle Off",
            "move",
            "stop false",
            "move",
            "stop true",
            "move",
            "end true"
        ]
    );
}

#[test]
fn spindle_and_coolant_before_the_move() {
    let r = run("G71 X1 M3\nX2 M8\nX3 M7\n");
    assert_eq!(
        kinds(&r),
        vec![
            "spindle Cw",
            "move",
            "coolant Flood",
            "move",
            "coolant Mist",
            "move"
        ]
    );
}

#[test]
fn m13_and_m14_start_spindle_and_coolant() {
    let r = run("M13\nM14\n");
    assert_eq!(
        r.commands,
        vec![
            Command::SetSpindleState {
                spindle: 0,
                state: SpindleState::Cw
            },
            Command::Coolant(CoolantState::Flood),
            Command::SetSpindleState {
                spindle: 0,
                state: SpindleState::Ccw
            },
            Command::Coolant(CoolantState::Flood),
        ]
    );
}

#[test]
fn m19_stops_the_spindle() {
    let r = run("M3\nM19\n");
    assert_eq!(kinds(&r), vec!["spindle Cw", "spindle Off"]);
}

#[test]
fn m2_ends_without_rewind() {
    assert_eq!(kinds(&run("M2\n")), vec!["end false"]);
}

#[test]
fn tool_change_uses_the_tool_code_before_the_point() {
    // "T9.9 M6": tool 9, offset 9 (manual p. 1-8 and the U-axis example).
    let r = run("T9.9 M6\n");
    assert!(r.commands.contains(&Command::SelectTool(9)));
    assert_eq!(kinds(&r), vec!["tool 9"]);
}
