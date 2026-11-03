#!/usr/bin/env bash
# Checks the Linux packages Tauri built: each holds the program, the menu
# entry (opening a file given to it, offering itself for G-code files),
# the G-code file type, the icons and the licence, and asks for WebKitGTK.
# Run by CI after building; needs dpkg-deb, rpm2cpio and cpio.
#
# Usage: packaging/linux/check-packages.sh FILE.deb FILE.rpm
set -euo pipefail
deb="$(realpath "$1")"
rpm_file="$(realpath "$2")"
failures=0
fail() {
  echo "FAIL: $*"
  failures=$((failures + 1))
}

# has <list> <description> <pattern>: the list has a line matching pattern.
has() {
  if ! grep -Eq -- "$3" <<< "$1"; then fail "$2"; fi
}

check_files() {
  local kind="$1" list="$2"
  has "$list" "$kind: the program in /usr/bin" '(^|/)usr/bin/gcode-editsim$'
  has "$list" "$kind: a menu entry" '(^|/)usr/share/applications/gcode-editsim\.desktop$'
  has "$list" "$kind: the G-code file type" '(^|/)usr/share/mime/packages/com\.ncsoftbyai\.gcodeeditsim\.xml$'
  has "$list" "$kind: the scalable icon" '(^|/)usr/share/icons/hicolor/scalable/apps/com\.ncsoftbyai\.gcodeeditsim\.svg$'
  has "$list" "$kind: the licence" '(^|/)usr/share/licenses/gcode-editsim/LICENSE$'
}

check_desktop() {
  local kind="$1" entry="$2"
  has "$entry" "$kind: the menu entry is called GCode EditSim" '^Name=GCode EditSim$'
  has "$entry" "$kind: the menu entry passes the file it is given (%f)" '^Exec=gcode-editsim %f$'
  has "$entry" "$kind: the menu entry offers the app for G-code files" '^MimeType=text/x-cnc-gcode;$'
  has "$entry" "$kind: the menu entry uses the app's icon" '^Icon=com\.ncsoftbyai\.gcodeeditsim$'
  # The app sets its window class to its identifier (app/shell/src/lib.rs),
  # so the desktop can match the window to this entry and its icon.
  has "$entry" "$kind: the menu entry names the window class" '^StartupWMClass=com\.ncsoftbyai\.gcodeeditsim$'
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
# Each package unpacked, its files listed from there (names may hold
# spaces: Tauri names the menu entry after the app, "GCode EditSim").
files_in() { (cd "$1" && find . -type f | sed 's|^\./||'); }

# The .deb
mkdir "$tmp/deb"
dpkg-deb --extract "$deb" "$tmp/deb"
check_files deb "$(files_in "$tmp/deb")"
check_desktop deb "$(cat "$tmp"/deb/usr/share/applications/*.desktop)"
has "$(dpkg-deb --field "$deb" Package)" "deb: the package is called gcode-editsim" '^gcode-editsim$'
has "$(dpkg-deb --field "$deb" Depends)" "deb: depends on WebKitGTK 4.1" 'libwebkit2gtk-4\.1'

# The .rpm
mkdir "$tmp/rpm"
# rpm2cpio exits 1 on the packages Tauri writes although it writes the
# whole archive (rpm 4.18); whether everything came out is checked below.
(cd "$tmp/rpm" && { rpm2cpio "$rpm_file" || true; } | cpio --extract --make-directories --quiet)
check_files rpm "$(files_in "$tmp/rpm")"
check_desktop rpm "$(cat "$tmp"/rpm/usr/share/applications/*.desktop)"
has "$(rpm --query --queryformat '%{NAME}' --package "$rpm_file" 2> /dev/null)" "rpm: the package is called gcode-editsim" '^gcode-editsim$'
has "$(rpm --query --requires --package "$rpm_file" 2> /dev/null)" "rpm: requires WebKitGTK 4.1" 'webkit2gtk4\.1|libwebkit2gtk-4\.1'

if [ "$failures" -gt 0 ]; then
  echo "$failures problem(s) in the Linux packages"
  exit 1
fi
echo "Linux packages: all checks passed"
