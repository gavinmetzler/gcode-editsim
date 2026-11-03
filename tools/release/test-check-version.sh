#!/usr/bin/env bash
# Tests for check-version.sh. Run: tools/release/test-check-version.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0

# A copy of the two files the script reads, at versions $1 (tauri.conf.json)
# and $2 (Cargo.toml).
repo() {
  rm -rf "$tmp/r"
  mkdir -p "$tmp/r/app/shell"
  printf '{\n  "productName": "X",\n  "version": "%s",\n  "build": {}\n}\n' "$1" > "$tmp/r/app/shell/tauri.conf.json"
  printf '[package]\nname = "x"\nversion = "%s"\n\n[dependencies]\ny = { version = "9.9.9" }\n' "$2" > "$tmp/r/app/shell/Cargo.toml"
}

# expect <ok|fail> <tag>
expect() {
  local got=ok
  "$here/check-version.sh" "$2" "$tmp/r" > /dev/null 2>&1 || got=fail
  if [ "$got" != "$1" ]; then
    echo "FAIL: tag [$2] expected $1, got $got"
    failures=$((failures + 1))
  fi
}

repo 0.2.0 0.2.0
expect ok v0.2.0
expect fail v0.2.1
expect fail 0.2.0      # tags start with v
expect fail v0.2       # not the same version
expect fail ""
repo 0.2.0 0.1.0       # the two files disagree
expect fail v0.2.0
expect fail v0.1.0
repo 1.0.0-rc.1 1.0.0-rc.1
expect ok v1.0.0-rc.1

# --print-tag (the Run workflow button): the tag for the app's version,
# when the two files agree.
# expect_tag <expected output, or "fail">
expect_tag() {
  local got
  got=$("$here/check-version.sh" --print-tag "$tmp/r" 2> /dev/null) || got=fail
  if [ "$got" != "$1" ]; then
    echo "FAIL: --print-tag expected [$1], got [$got]"
    failures=$((failures + 1))
  fi
}
repo 0.5.0 0.5.0
expect_tag v0.5.0
repo 0.5.0 0.4.0
expect_tag fail

# The repository's own files agree with each other.
real="$(cd "$here/../.." && pwd)"
v=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' "$real/app/shell/tauri.conf.json")
"$here/check-version.sh" "v$v" "$real" > /dev/null || { echo "FAIL: the repository's versions disagree"; failures=$((failures + 1)); }

if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "check-version: all tests passed"
