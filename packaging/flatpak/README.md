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

The app also appears in your desktop's app list. Your machine files
are in `~/.var/app/com.ncsoftbyai.gcodeeditsim/config/com.ncsoftbyai.gcodeeditsim/machines/`.
Flatpak cannot run anything at install time, so the app makes that folder
the first time it starts and copies the example mill and router files
into it (they are packaged in the app, not downloaded). Edit them or add
your own, then use **Machines… → Reload list**. If you delete every file,
the examples come back on the next start.

### Opening G-code files from the file manager

The app registers a file type, **CNC G-code program**
(`text/x-cnc-gcode`), for `*.nc`, `*.ngc` and `*.tap` files (any letter
case), shown with the app's icon, and says it opens them. Double-clicking
such a file, or **Open With → GCode EditSim**, starts the app with that
file instead of the sample.

**Making it the default.** A Flatpak runs nothing when it is installed, so
it cannot make itself the default app for a file type. Desktops use it
by default when it is the only app for the type; otherwise choose it once:

- **KDE (Dolphin):** right-click a `.nc` file → **Open With → Other
  Application…** → choose GCode EditSim and tick **Remember application
  association for all files of type "CNC G-code program"**. Or **System
  Settings → Apps & Windows → Default Applications → File Associations**,
  find `text/x-cnc-gcode` and move GCode EditSim to the top.
- **GNOME (Files):** right-click a `.nc` file → **Open With…** → choose
  GCode EditSim and turn on **Always use for this file type**.

If the icon or the type does not show straight after installing, log out
and in once (desktops cache the file types).

**NetCDF files keep their type.** `*.nc` is also the extension of NetCDF
scientific data files. They are binary and start with a known signature,
so the desktop tells them apart by content: G-code text is the G-code
type, NetCDF files stay NetCDF (and `*.ngc` Neo Geo Pocket Color ROMs stay
ROMs). `check-mime.sh` checks this with GNOME's GIO and, if PyQt6 is
installed, KDE's Qt (see the comment in `com.ncsoftbyai.gcodeeditsim.xml`
for why the package also defines those two types).

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

M2.8b (native dialogs, `tauri-plugin-dialog`), the same way:

- **Save As…** and **Open…** show the desktop's file chooser through the
  portal (GTK's native chooser does this by itself inside a sandbox);
- Save As wrote a new file on the host, and **Save** after an edit
  replaced it in place (the app's save writes a temporary file and renames
  it, which the document portal allows);
- a file with Windows (CRLF) line endings kept them after an edit and a
  save, and no temporary file was left behind.

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
| `com.ncsoftbyai.gcodeeditsim.xml` | The G-code file type (M4.11) |
| `check-mime.sh` | Checks how GIO and Qt resolve the file types |
| `../icons/com.ncsoftbyai.gcodeeditsim.svg` | App icon (M4.10): an editor page beside a backplot |

The app's PNG icons (`app/shell/icons/`, embedded by Tauri as the window
icon) are rendered from that SVG. After changing it, regenerate them from
the repository root with `rsvg-convert` (Debian/Ubuntu package
`librsvg2-bin`):

```bash
for s in 32 128 256 512; do
  case $s in 32) f=32x32.png;; 128) f=128x128.png;; 256) f=128x128@2x.png;; 512) f=icon.png;; esac
  rsvg-convert -w $s -h $s packaging/icons/com.ncsoftbyai.gcodeeditsim.svg -o app/shell/icons/$f
done
```

A test in `app/shell/tests/app_id.rs` checks the four sizes.
| `build.sh` | Build script |
