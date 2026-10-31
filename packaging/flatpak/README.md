# Flatpak package

GCode EditSim is distributed as a single `.flatpak` file that users
download and install themselves. It is not on Flathub.

## Installing (users)

You need Flatpak and the Flathub remote, which provides the shared GNOME
runtime the app runs on. Most Linux desktops have both; otherwise see
<https://flatpak.org/setup/>.

```bash
flatpak install --user gcode-editsim.flatpak
flatpak run com.ncsoftbyai.gcodeeditsim
```

The app also appears in your desktop's app list. Your own machine files
go in `~/.var/app/com.ncsoftbyai.gcodeeditsim/config/com.ncsoftbyai.gcodeeditsim/machines/`.

## Building

Needs `flatpak`, `flatpak-builder`, `elfutils` (for `eu-strip`),
`librsvg2-common` (to render the SVG icon), Node.js 22 (npm) and Rust
(cargo). Most desktops have the last two system packages already.

```bash
packaging/flatpak/build.sh            # writes ./gcode-editsim.flatpak
```

What it does:

1. Builds the UI (`ui/dist`, plain static files).
2. Vendors the Rust crates (`cargo vendor`) into `packaging/flatpak/.build/`.
3. Runs `flatpak-builder` on `com.ncsoftbyai.gcodeeditsim.yml`. The
   build inside the sandbox is offline: it only compiles the app, against
   the GNOME runtime's own WebKitGTK.
4. Bundles the result into one `.flatpak` file.

## CI

The `Flatpak bundle` job in `.github/workflows/ci.yml` runs `build.sh` on
every pull request and push to `main`. It keeps `gcode-editsim.flatpak`
for 30 days as the `gcode-editsim-flatpak` artifact of the run: download it
from the run's summary page on GitHub.

## Sandbox

The app gets a window (Wayland, or X11 as a fallback), the GPU for the 3D
view, and nothing else: **no network and no access to your files**. Files
are opened through the desktop's file chooser (the portal), which grants
access to the chosen file only.

## Tested

The bundle was built with `build.sh` and installed (GNOME runtime 51).
Then, inside its sandbox, under a virtual display with a desktop session's
portal services:

- the app starts, with the editor, the toolpath and line sync;
- **Open…** shows the desktop's file chooser through the portal;
- the chosen file reaches the sandbox through the document portal and is
  read, run and drawn.

On the owner's desktop (Wayland, NVIDIA graphics) the first bundle closed
at start with a Wayland protocol error; see "Known issues" for the cause
and the setting that fixes it.

## Known issues

- **Window closes at start with "Error 71 (Protocol error) dispatching to
  Wayland display"** (seen with NVIDIA graphics on Wayland): WebKitGTK's
  DMA-BUF renderer fails there. The package now sets
  `WEBKIT_DISABLE_DMABUF_RENDERER=1`, which avoids it. To try the faster
  renderer anyway: `flatpak run --env=WEBKIT_DISABLE_DMABUF_RENDERER=0
  com.ncsoftbyai.gcodeeditsim`.
- **Forcing X11** (`--socket=x11 --env=GDK_BACKEND=x11`) failed to start
  GTK on the same machine. The app runs on Wayland natively, so this is not
  needed; X11 is used automatically only where there is no Wayland.

## Files

| File | What it is |
|---|---|
| `com.ncsoftbyai.gcodeeditsim.yml` | Flatpak manifest |
| `com.ncsoftbyai.gcodeeditsim.desktop` | Desktop entry |
| `com.ncsoftbyai.gcodeeditsim.metainfo.xml` | AppStream metadata (description, licence, releases) |
| `../icons/com.ncsoftbyai.gcodeeditsim.svg` | App icon (**placeholder**) |
| `build.sh` | Build script |
