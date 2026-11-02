#!/bin/sh
# Checks how the desktop resolves the file types in
# com.ncsoftbyai.gcodeeditsim.xml (M4.11): G-code text named *.nc, *.ngc
# or *.tap must be text/x-cnc-gcode, while real NetCDF files (*.nc) and
# Neo Geo Pocket Color ROMs (*.ngc) keep their own types. Installs the
# package into a temporary folder placed before /usr/share, as Flatpak's
# exported folder is, and asks GNOME's GIO and, if PyQt6 is installed,
# KDE's Qt (QMimeDatabase, what Dolphin uses).
#
# Needs: update-mime-database, gio; optionally python3 with PyQt6.
# Prints each file's type and exits non-zero if any is wrong.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/xdg/mime/packages" "$tmp/files"
cp "$here/com.ncsoftbyai.gcodeeditsim.xml" "$tmp/xdg/mime/packages/"
update-mime-database "$tmp/xdg/mime" 2>/dev/null
cd "$tmp/files"
printf 'G21 G90 G0 X0 Y0\nG1 X10. F100.\n' > part.nc
cp part.nc PART.NC; cp part.nc part.ngc; cp part.nc part.tap
printf 'CDF\001\000\000\000\000\000\000\000\000' > classic.nc
printf 'CDF\002\000\000\000\000\000\000\000\000' > offset64.nc
printf '\211HDF\r\n\032\n\000\000\000\000\000\010\010\000' > netcdf4.nc
# A ROM header: the SNK copyright at 0 and 0x10 at offset 35.
printf 'COPYRIGHT BY SNK CORPORATION\000\000\000\000\000\000\000\020\000\000\000\000' > rom.ngc
expect() {
  case $1 in
    classic.nc|offset64.nc|netcdf4.nc) echo application/x-netcdf ;;
    rom.ngc) echo application/x-neo-geo-pocket-color-rom ;;
    *) echo text/x-cnc-gcode ;;
  esac
}
export XDG_DATA_DIRS="$tmp/xdg:/usr/share"
status=0
echo "GIO (GNOME):"
for f in part.nc PART.NC part.ngc part.tap classic.nc offset64.nc netcdf4.nc rom.ngc; do
  got=$(gio info -a standard::content-type "$f" | awk '/content-type/ {print $2}')
  want=$(expect "$f")
  mark=ok; [ "$got" = "$want" ] || { mark=WRONG; status=1; }
  printf '  %-12s %-42s %s\n' "$f" "$got" "$mark"
done
for py in python3.12 python3; do
  if command -v "$py" >/dev/null && "$py" -c 'import PyQt6.QtCore' 2>/dev/null; then
    echo "Qt (KDE, Dolphin):"
    for f in part.nc PART.NC part.ngc part.tap classic.nc offset64.nc netcdf4.nc rom.ngc; do
      got=$("$py" -c 'import sys; from PyQt6.QtCore import QMimeDatabase; print(QMimeDatabase().mimeTypeForFile(sys.argv[1]).name())' "$f")
      want=$(expect "$f")
      mark=ok; [ "$got" = "$want" ] || { mark=WRONG; status=1; }
      printf '  %-12s %-42s %s\n' "$f" "$got" "$mark"
    done
    break
  fi
done
exit $status
