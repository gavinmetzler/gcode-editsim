//! Writes `ui/src/generated/render_buffer.ts` (see the library docs).

#![forbid(unsafe_code)]

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(gen_ts::OUTPUT);
    let write = || -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, gen_ts::generate())
    };
    match write() {
        Ok(()) => {
            println!("wrote {}", gen_ts::OUTPUT);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            ExitCode::FAILURE
        }
    }
}
