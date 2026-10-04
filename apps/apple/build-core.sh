#!/bin/bash
# Builds Lumenna's Rust core for an Apple platform and regenerates its Swift bindings.
#
#   apps/apple/build-core.sh [iphonesimulator|iphoneos] [Debug|Release]
#
# Xcode runs this before every build, passing nothing: it reads PLATFORM_NAME and
# CONFIGURATION from Xcode's environment. Cargo does nothing when nothing changed, so an
# unchanged core costs a second or two.
#
# Cargo runs in a clean environment. Xcode exports SDKROOT, deployment targets and linker
# variables for the app's platform, and Cargo's build scripts — which compile for the Mac —
# pick them up and fail in confusing ways.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PLATFORM="${1:-${PLATFORM_NAME:-iphonesimulator}}"
CONFIGURATION="${2:-${CONFIGURATION:-Debug}}"

case "$PLATFORM" in
  iphonesimulator) TARGET=aarch64-apple-ios-sim ;;
  iphoneos) TARGET=aarch64-apple-ios ;;
  *) echo "error: no Rust target for platform $PLATFORM" >&2; exit 1 ;;
esac
case "$CONFIGURATION" in
  Release) PROFILE_FLAG=--release ;;
  *) PROFILE_FLAG= ;;
esac

DEVELOPER="${DEVELOPER_DIR:-$(xcode-select -p)}"
cargo() {
  env -i HOME="$HOME" USER="${USER:-}" \
    PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin" \
    DEVELOPER_DIR="$DEVELOPER" IPHONEOS_DEPLOYMENT_TARGET=17.0 \
    cargo "$@"
}

cd "$ROOT"
cargo build -p lumenna-ffi --lib $PROFILE_FLAG --target "$TARGET"

# The bindings come from the interface compiled into a host build of the same crate.
cargo build -p lumenna-ffi --lib
GENERATED="$ROOT/apps/apple/Generated"
mkdir -p "$GENERATED"
cargo run -q -p lumenna-ffi --features bindgen --bin uniffi-bindgen -- \
  generate --library "$ROOT/target/debug/liblumenna_ffi.dylib" \
  --language swift --out-dir "$GENERATED/bindings"

# Swift imports the C half as the module LumennaCoreFFI (named in crates/surface/uniffi.toml);
# give its map the name Clang looks for in a directory on the import path.
mkdir -p "$GENERATED/include"
cp "$GENERATED/bindings/LumennaCoreFFI.h" "$GENERATED/include/"
cp "$GENERATED/bindings/LumennaCoreFFI.modulemap" "$GENERATED/include/module.modulemap"
