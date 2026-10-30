# Android spike (M2.3): results and how to reproduce

**Throwaway branch. Not for `main`.**

## Result

On a real Android phone (arm64), the self-test page (`placeholder/selftest.js`)
reported **ALL PASSED** (owner, 2026-10-30):

- `interpret` took the program as a raw request body and returned the render
  buffer as an `ArrayBuffer`, byte-identical in every checked value to the
  desktop build: magic, version, counts, the f64 origin, a vertex read through
  a zero-copy `Float32Array` view, segment kinds and source lines.
- A 20 000-block program ran through the same path.
- `save_file`/`load_file` round-tripped all 256 byte values to a path
  containing "ü" (percent-encoded `x-path` header).
- The Rust core builds for `aarch64-linux-android` with `#![forbid(unsafe_code)]`.

So the binary boundary of ARCHITECTURE.md §8 works on Android as on desktop.
Desktop (Linux, WebKitGTK, under Xvfb): same checks, all passed; the
20 000-block program took 258 ms.

## What the build needed

| Item | Version used |
|---|---|
| Android SDK command-line tools | 9862592 (`/opt/android-sdk`) |
| NDK | 29.0.14206865 (latest stable before 30) |
| Platform / build tools | android-36 / 36.1.0 |
| JDK | 21 (system) |
| Tauri CLI | `@tauri-apps/cli` 2.12.4 (npm, see `package.json`) |
| Rust target | `aarch64-linux-android` (`tauri android init` adds the others) |

Changes to the app for mobile (all in this branch):

- `Cargo.toml`: `crate-type = ["staticlib", "cdylib", "rlib"]`.
- `src/lib.rs`: a `#[tauri::mobile_entry_point]` function calling `run()`.
- `package.json`: the Tauri CLI. The generated Gradle project calls back into
  it with `npm run tauri`, so it must be run that way.

Network: the build downloads from `dl.google.com` (SDK, NDK, Android Gradle
plugin), `services.gradle.org`, `plugins.gradle.org` and Maven Central.
Maven Central answered HTTP 429 (rate limit) from the cloud environment;
`maven-central-mirror.gradle` (copy to `~/.gradle/init.d/`) points Gradle at
Google's official Maven Central mirror instead.

## Reproduce

```bash
export ANDROID_HOME=/opt/android-sdk NDK_HOME=$ANDROID_HOME/ndk/29.0.14206865
cd app/shell
npm install
npm run tauri -- android init --ci
# Smaller APK: no debug info in the Rust library.
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_STRIP=symbols \
  npm run tauri -- android build --target aarch64 --debug --apk --ci
# -> gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

## Notes for M3

- iOS needs a Mac with Xcode; not attempted.
- No emulator here (no `/dev/kvm`): testing needs a real device or an
  emulator on a developer machine.
- A debug build with debug info made a 146 MB APK (139 MB of it the Rust
  library's debug info); without it, 20 MB. Release builds need a signing
  key, not set up.
- `gen/android` is generated (ignored here). Whether to commit it for M3 is
  open; Tauri's docs suggest committing it once customised.
