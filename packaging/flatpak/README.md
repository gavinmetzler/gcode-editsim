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

Needs `flatpak`, `flatpak-builder`, Node.js 22 (npm) and Rust (cargo).

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

Not yet tried on a real desktop: GPU rendering (the virtual display has
none), Wayland, and installing from the downloaded file on another
machine.

## Files

| File | What it is |
|---|---|
| `com.ncsoftbyai.gcodeeditsim.yml` | Flatpak manifest |
| `com.ncsoftbyai.gcodeeditsim.desktop` | Desktop entry |
| `com.ncsoftbyai.gcodeeditsim.metainfo.xml` | AppStream metadata (description, licence, releases) |
| `../icons/com.ncsoftbyai.gcodeeditsim.svg` | App icon (**placeholder**) |
| `build.sh` | Build script |
