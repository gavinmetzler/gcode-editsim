# app

The desktop app, in two parts:

- `backend/`: the app's behaviour in plain Rust (choosing the machine and
  dialect, interpreting a program into a render buffer, file access). It is
  in the main Cargo workspace and tested there.
- `shell/`: the Tauri 2 shell, a thin command layer over `backend` that
  shows the UI from `ui/dist`. It is its own Cargo workspace, so the rest
  builds without WebKit. Build the UI first, then `cargo run` here.

See `ARCHITECTURE.md` §8.4a for the commands and the README's
"Trying it out" for setup.
