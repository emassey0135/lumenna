#!/bin/bash
# Writes Generated/Metadata.xcconfig from the workspace's Cargo.toml, `[workspace.metadata.lumenna]`,
# the one place what Lumenna is is said (crates/records' build reads the same): the version as
# MARKETING_VERSION, which every Info.plist's CFBundleShortVersionString takes; the name as
# LUMENNA_NAME, their CFBundleDisplayName; and the identifier Apple's bundles share as
# LUMENNA_PACKAGE_ID, on which each target's bundle identifier and App Group build. A version
# bump is that one edit.
#
#   apps/apple/metadata.sh
#
# XcodeGen runs it before generating (project.yml's preGenCommand), since the project reads the
# file; build-core.sh runs it again on every build and stops one built with what Cargo.toml no
# longer says. The file is rewritten only when what it says changed, so Xcode sees no change
# otherwise.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/apps/apple/Generated/Metadata.xcconfig"

# A key of `[workspace.metadata.lumenna]`, its quotes taken off.
field() {
  awk -v key="$1" '
    /^\[/ { section = $0; next }
    section == "[workspace.metadata.lumenna]" {
      split($0, part, "=")
      name = part[1]; gsub(/[ \t]/, "", name)
      if (name == key) {
        value = substr($0, index($0, "=") + 1)
        gsub(/^[ \t]*"|"[ \t]*$/, "", value)
        print value
        exit
      }
    }
  ' "$ROOT/Cargo.toml"
}

VERSION="$(field version)"
NAME="$(field name)"
PACKAGE_ID="$(field package_id)"
[ -n "$VERSION" ] && [ -n "$NAME" ] && [ -n "$PACKAGE_ID" ] || { echo "error: Cargo.toml's [workspace.metadata.lumenna] has no version, name or package_id" >&2; exit 1; }

TEXT="// Written by apps/apple/metadata.sh from the workspace's Cargo.toml: change it there.
MARKETING_VERSION = $VERSION
LUMENNA_NAME = $NAME
LUMENNA_PACKAGE_ID = $PACKAGE_ID"

mkdir -p "$(dirname "$OUT")"
[ -f "$OUT" ] && [ "$(cat "$OUT")" = "$TEXT" ] || printf '%s\n' "$TEXT" > "$OUT"
