#!/usr/bin/env bash
# Checks that a release tag names the app's version: `v` followed by the
# version in app/shell/tauri.conf.json (what the packages are called and
# what the app reports), which must also be the version in
# app/shell/Cargo.toml. Run by the release workflow before publishing.
#
# Usage: tools/release/check-version.sh TAG [REPOSITORY_ROOT]
#        tools/release/check-version.sh --print-tag [REPOSITORY_ROOT]
# The second form prints the tag for the app's version (`v0.5.0`), after
# checking the two files agree: the release workflow's Run workflow
# button tags the release with it.
set -euo pipefail
tag="${1-}"
root="${2:-$(cd "$(dirname "$0")/../.." && pwd)}"

# The first "version" key of each file: the package's own, not a
# dependency's (those come later, inside tables or braces).
conf=$(sed -n 's/^  "version": "\(.*\)",\{0,1\}$/\1/p' "$root/app/shell/tauri.conf.json" | head -n 1)
cargo=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/app/shell/Cargo.toml" | head -n 1)

if [ -z "$conf" ] || [ -z "$cargo" ]; then
  echo "error: could not read the version from app/shell/tauri.conf.json or app/shell/Cargo.toml" >&2
  exit 1
fi
if [ "$conf" != "$cargo" ]; then
  echo "error: app/shell/tauri.conf.json says $conf but app/shell/Cargo.toml says $cargo" >&2
  exit 1
fi
if [ "$tag" = "--print-tag" ]; then
  echo "v$conf"
  exit 0
fi
if [ "$tag" != "v$conf" ]; then
  echo "error: the tag is '$tag' but the app's version is $conf: tag it v$conf, or change the version in both files first" >&2
  exit 1
fi
echo "version $conf matches tag $tag"
