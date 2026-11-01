#!/usr/bin/env bash
# Tests for changed-areas.sh. Run: tools/ci/test-changed-areas.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
failures=0

# expect "<files, one per line>" "<expected areas that are true, space-separated>"
expect() {
  local got
  got=$(printf '%s' "$1" | "$here/changed-areas.sh" | awk -F= '$2 == "true" { printf "%s%s", sep, $1; sep = " " }')
  if [ "$got" != "$2" ]; then
    echo "FAIL: for [$(printf '%s' "$1" | tr '\n' ',')] expected [$2], got [$got]"
    failures=$((failures + 1))
  fi
}

expect "README.md" ""
expect $'docs/plans/M2_PLAN.md\nDEVELOPMENT_STATUS.md\n.gitignore' ""
expect "packaging/flatpak/README.md" ""
expect "packaging/flatpak/com.ncsoftbyai.gcodeeditsim.yml" "flatpak"
expect "LICENSE" "flatpak"
expect "core/ir/src/render.rs" "rust app flatpak"
expect "core/gcode-syntax/src/parser.rs" "rust fuzz app flatpak"
expect "tests/golden/basic_mill.nc" "rust fuzz ui app flatpak"
expect "Cargo.lock" "rust app flatpak audit"
expect "rust-toolchain.toml" "rust app flatpak"
expect "app/backend/src/lib.rs" "rust app flatpak"
expect "app/shell/src/lib.rs" "app flatpak"
expect "tools/depcheck/rules.toml" "rust"
expect "ui/src/main.ts" "ui app flatpak"
expect "ui/src/generated/render_buffer.ts" "rust ui app flatpak"
expect $'README.md\nui/src/main.ts' "ui app flatpak"
expect ".github/workflows/ci.yml" "rust fuzz ui app flatpak audit"
expect "tools/ci/changed-areas.sh" "rust fuzz ui app flatpak audit"
# Anything unknown runs everything.
expect "newfolder/thing.txt" "rust fuzz ui app flatpak audit"
expect "build.rs" "rust fuzz ui app flatpak audit"
expect "app/shell/Cargo.lock" "app flatpak audit"
expect "core/gcode-syntax/fuzz/Cargo.lock" "rust fuzz app flatpak audit"
expect "ui/package-lock.json" "ui app flatpak audit"
expect "ui/package.json" "ui app flatpak"
# No files: nothing to do.
expect "" ""

if [ "$failures" -gt 0 ]; then
  echo "$failures test(s) failed"
  exit 1
fi
echo "changed-areas: all tests passed"
