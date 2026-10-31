#!/usr/bin/env bash
# Builds gcode-editsim.flatpak, a single-file bundle users install with
#   flatpak install --user gcode-editsim.flatpak
# Needs: flatpak, flatpak-builder, elfutils, Node 22 (npm), Rust (cargo), and the
# Flathub remote for the GNOME runtime and SDK.
# FLATPAK_BUILDER_ARGS adds options to flatpak-builder (CI uses
# --disable-rofiles-fuse).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
app_id=com.ncsoftbyai.gcodeeditsim
build="$here/.build"
out="${1:-$root/gcode-editsim.flatpak}"

mkdir -p "$build"

echo "== UI: build the static files (ui/dist)"
(cd "$root/ui" && npm ci && npm run build)

echo "== Rust: vendor the crates so the sandboxed build is offline"
rm -rf "$build/vendor"
(cd "$root/app/shell" && cargo vendor --locked --versioned-dirs "$build/vendor" >/dev/null)

echo "== Flatpak: build, then bundle"
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
# shellcheck disable=SC2086 # FLATPAK_BUILDER_ARGS is a list of options.
flatpak-builder --user --force-clean --install-deps-from=flathub ${FLATPAK_BUILDER_ARGS:-} \
  --repo="$build/repo" --state-dir="$build/state" "$build/app" "$here/$app_id.yml"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  "$build/repo" "$out" "$app_id"

echo "== Wrote $out"
