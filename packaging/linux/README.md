# Linux packages (.deb and .rpm)

Built by CI with Tauri's bundler (`.github/workflows/packages.yml`) and
published with every release (`.github/workflows/release.yml`), with
fixed names so the README's "latest" links always work:

| File | For |
|---|---|
| `gcode-editsim-amd64.deb` | Ubuntu 22.04 and newer, Debian 12 and newer, Linux Mint, Pop!_OS, … |
| `gcode-editsim-x86_64.rpm` | Fedora, openSUSE, RHEL 10 and others with WebKitGTK 4.1 |

They are built on Ubuntu 22.04, so they run on that and anything newer
(a program links against the C library of the system it was built on).
64-bit x86 only for now.

## Installing (users)

```bash
sudo apt install ./gcode-editsim-amd64.deb    # Debian, Ubuntu and relatives
sudo dnf install ./gcode-editsim-x86_64.rpm   # Fedora and relatives
```

The package manager installs WebKitGTK and GTK with it if they are
missing. The app appears in the desktop's app list as **GCode EditSim**
and in a terminal as `gcode-editsim`. Remove it with `sudo apt remove
gcode-editsim` or `sudo dnf remove gcode-editsim`.

Your machine files are in `~/.config/com.ncsoftbyai.gcodeeditsim/machines/`:
the app makes that folder the first time it starts and copies the example
mill and router files into it. Your settings and machine files stay when
the package is removed.

The packages register the **CNC G-code program** file type
(`text/x-cnc-gcode`, `*.nc`, `*.ngc`, `*.tap`) and say the app opens it,
like the Flatpak (`packaging/flatpak/README.md`, which explains why
NetCDF `.nc` files keep their own type). Installing both the Flatpak and a
package gives two entries in the app list; keep one.

## What is in them

- `/usr/bin/gcode-editsim`, the app (the UI is built into it).
- The menu entry, `/usr/share/applications/gcode-editsim.desktop`, made
  from `desktop.hbs`: it passes the file it is given (`%f`), offers the
  app for G-code files, and names the window class the app sets (its
  identifier), so the desktop shows the app's icon on its window.
- The G-code file type (`packaging/flatpak/com.ncsoftbyai.gcodeeditsim.xml`,
  shared with the Flatpak), the icons and the licence.
- Dependencies: WebKitGTK 4.1 and GTK 3 (worked out by the bundler).

`package-name.conf.json` names the packages `gcode-editsim`; without it
Tauri would make `g-code-edit-sim` from the product name. It is given to
the bundler only; the app's settings folder goes by its identifier, so it
does not change.

## Building locally

```bash
cargo install tauri-cli --version 2.12.4 --locked   # once
cd ui && npm ci && npm run build && cd ..
cd app/shell && cargo tauri build --bundles deb,rpm --config ../../packaging/linux/package-name.conf.json
```

The packages are in `app/shell/target/release/bundle/deb/` and `rpm/`.
`check-packages.sh FILE.deb FILE.rpm` checks what they hold (CI runs it,
then installs the `.deb`).
