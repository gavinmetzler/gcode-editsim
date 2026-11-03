# Windows and macOS packages

Built by CI with Tauri's bundler (`.github/workflows/packages.yml`, jobs
`windows` and `macos`) and published with every release, with fixed names
so the README's "latest" links always work. Their settings are
`app/shell/tauri.windows.conf.json` and `app/shell/tauri.macos.conf.json`,
which Tauri adds to `tauri.conf.json` when building on that system.

## Windows: `gcode-editsim-windows-x64-setup.exe`

- An NSIS installer for the current user: no administrator rights, the
  app goes in `%LOCALAPPDATA%\GCode EditSim` and gets a Start menu entry.
  Remove it in **Settings → Apps**.
- It registers `.nc`, `.ngc` and `.tap` files as **CNC G-code program**,
  opened by the app (Windows keeps a default app you chose yourself; use
  **Open with** to change it).
- The app draws with Microsoft's WebView2, which Windows 10 and 11
  normally have; the installer downloads it if not.
- Machine files and settings: `%APPDATA%\com.ncsoftbyai.gcodeeditsim\`
  (`machines\` holds the machine files; the app copies the example mill
  and router files there the first time it starts).
- Not signed with a code-signing certificate, so SmartScreen may warn:
  **More info → Run anyway**.

CI installs it silently (`/S`), checks the app and the `.nc` file type are
there, and starts the app.

## macOS: `gcode-editsim-macos.dmg`

- One app for Apple silicon and Intel Macs (a universal binary), macOS 11
  and newer. Open the disk image and drag **GCode EditSim** to
  **Applications**.
- Signed ad hoc, not with an Apple developer certificate (which costs a
  yearly fee) and not notarised, so macOS asks before the first start:
  **System Settings → Privacy & Security → Open Anyway** (macOS 15), or
  Control-click → **Open** (macOS 14 and older).
- Machine files and settings:
  `~/Library/Application Support/com.ncsoftbyai.gcodeeditsim/`.
- **Not yet:** opening a G-code file from the Finder. macOS hands the file
  to a running app as an event rather than on the command line, which the
  app does not handle yet; open files with **Open…** in the app. It is in
  `docs/WISHLIST.md`.

CI checks the app holds code for both processors, that its signature and
the disk image are valid, and starts it.

## Building locally

On the system itself (Tauri cannot build these from Linux without extra
tools):

```bash
cd ui && npm ci && npm run build && cd ..
cd app/shell
npx --yes @tauri-apps/cli@2.12.4 build --bundles nsis                                            # Windows
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npx --yes @tauri-apps/cli@2.12.4 build --bundles app,dmg --target universal-apple-darwin           # macOS
```
