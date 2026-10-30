//! GCode EditSim desktop app.

// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    match gcode_editsim::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("GCode EditSim could not start: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
