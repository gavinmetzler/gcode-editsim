#!/usr/bin/env bash
# Decides which CI jobs a change needs. Reads the changed file paths (one
# per line) on stdin and prints `area=true|false` for each area, in the
# format GitHub Actions reads from $GITHUB_OUTPUT.
#
# Conservative by design: a path no rule recognises turns every area on,
# so a new folder can never silently skip CI. Documentation-only changes
# turn every area off. Changes to the workflow or to this script turn
# everything on.
set -euo pipefail

areas=(rust fuzz ui app flatpak audit packages)
declare -A on
for a in "${areas[@]}"; do on[$a]=false; done
all() { for a in "${areas[@]}"; do on[$a]=true; done; }

while IFS= read -r path || [ -n "$path" ]; do
  [ -z "$path" ] && continue
  matched=false
  mark() { for a in "$@"; do on[$a]=true; done; matched=true; }
  case "$path" in
    .github/workflows/* | tools/ci/*) all; continue ;;
    # Documentation never changes a build, wherever it is.
    *.md | docs/* | .gitignore | */.gitignore) continue ;;
    # The release scripts feed only the packages.
    tools/release/*) on[packages]=true; continue ;;
  esac
  # The areas each path feeds (several can match).
  case "$path" in core/gcode-syntax/*) mark fuzz ;; esac
  case "$path" in tests/golden/*) mark rust fuzz ui app flatpak ;; esac
  case "$path" in core/* | Cargo.toml | Cargo.lock | rust-toolchain.toml) mark rust app flatpak ;; esac
  case "$path" in app/backend/* | tools/*) mark rust ;; esac
  case "$path" in ui/src/generated/*) mark rust ;; esac
  case "$path" in ui/*) mark ui app flatpak ;; esac
  case "$path" in app/*) mark app flatpak ;; esac
  case "$path" in packaging/* | LICENSE) mark flatpak ;; esac
  # The Linux packages (packages.yml): their settings and the files they
  # install. The rest of the app is built for them too, but the Flatpak
  # and app jobs already check that it builds.
  case "$path" in
    packaging/linux/* | packaging/icons/* | packaging/flatpak/*.xml | LICENSE | \
      app/shell/tauri.conf.json | app/shell/Cargo.toml | app/shell/Cargo.lock | app/shell/icons/*)
      mark packages ;;
  esac
  # Lock files say which dependency versions are used: audit them.
  case "$path" in Cargo.lock | */Cargo.lock | ui/package-lock.json) mark audit ;; esac
  if [ "$matched" = false ]; then all; fi
done

for a in "${areas[@]}"; do echo "$a=${on[$a]}"; done
