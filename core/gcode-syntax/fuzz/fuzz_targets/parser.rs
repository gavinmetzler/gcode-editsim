//! Feeds arbitrary bytes to the parser. Any panic, overflow or hang is a bug.
//!
//! Run from `core/gcode-syntax/`: `cargo +nightly fuzz run parser`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = gcode_syntax::parse(data);
});
