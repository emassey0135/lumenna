#!/bin/bash
# Builds Lumenna's Rust core for an Apple platform and regenerates its Swift bindings.
#
#   apps/apple/build-core.sh [iphonesimulator|iphoneos|macosx|watchsimulator|watchos] [Debug|Release]
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

# The Mac app is built for every architecture Xcode asks for (ARCHS: Apple silicon and
# Intel for a release, the Mac's own for a debug build) and the libraries joined into one.
ARCHS="${ARCHS:-arm64}"
# The watch's core has no network of its own: watchOS allows no sockets outside an audio
# session, so it syncs through its iPhone (crates/surface/src/link.rs), and Iroh is left out.
FEATURES=
GENERATED="$ROOT/apps/apple/Generated"
case "$PLATFORM" in
  watchsimulator|watchos) FEATURES=--no-default-features; GENERATED="$ROOT/apps/apple/Generated/Watch" ;;
esac
case "$PLATFORM" in
  iphonesimulator) TARGETS=aarch64-apple-ios-sim ;;
  iphoneos) TARGETS=aarch64-apple-ios ;;
  watchsimulator) TARGETS=aarch64-apple-watchos-sim ;;
  # Every watch watchOS 27 runs on is 64-bit, so arm64_32 (a tier 3 Rust target) is not built.
  watchos) TARGETS=aarch64-apple-watchos ;;
  # Named rather than left to the host build, so the Mac app's library is built against
  # its deployment target and kept apart from the one uniffi-bindgen reads.
  macosx)
    TARGETS=
    for arch in $ARCHS; do
      case "$arch" in
        arm64) TARGETS="$TARGETS aarch64-apple-darwin" ;;
        x86_64) TARGETS="$TARGETS x86_64-apple-darwin" ;;
        *) echo "error: no Rust target for the Mac architecture $arch" >&2; exit 1 ;;
      esac
    done
    ;;
  *) echo "error: no Rust target for platform $PLATFORM" >&2; exit 1 ;;
esac
case "$CONFIGURATION" in
  Release) PROFILE_FLAG=--release; PROFILE=release ;;
  *) PROFILE_FLAG=; PROFILE=debug ;;
esac

DEVELOPER="${DEVELOPER_DIR:-$(xcode-select -p)}"
cargo() {
  env -i HOME="$HOME" USER="${USER:-}" \
    PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin" \
    DEVELOPER_DIR="$DEVELOPER" IPHONEOS_DEPLOYMENT_TARGET=17.0 MACOSX_DEPLOYMENT_TARGET=14.0 \
    WATCHOS_DEPLOYMENT_TARGET=27.0 \
    cargo "$@"
}

cd "$ROOT"
for target in $TARGETS; do
  cargo build -p lumenna-ffi --lib $PROFILE_FLAG $FEATURES --target "$target"
done
if [ "$PLATFORM" = macosx ]; then
  # One library for every architecture asked for, where the Mac target links it from.
  UNIVERSAL="$ROOT/target/universal-apple-darwin/$PROFILE"
  mkdir -p "$UNIVERSAL"
  LIBS=()
  for target in $TARGETS; do LIBS+=("$ROOT/target/$target/$PROFILE/liblumenna_ffi.a"); done
  lipo -create "${LIBS[@]}" -output "$UNIVERSAL/liblumenna_ffi.a"
fi

# The bindings come from the interface compiled into a host build of the same crate, with
# the same features: the watch's in a target directory of its own, so the two host builds
# do not undo each other.
HOST_TARGET="$ROOT/target"
[ -n "$FEATURES" ] && HOST_TARGET="$ROOT/target/watch-host"
cargo build -p lumenna-ffi --lib $FEATURES --target-dir "$HOST_TARGET"
mkdir -p "$GENERATED"
cargo run -q -p lumenna-ffi $FEATURES --features bindgen --bin uniffi-bindgen --target-dir "$HOST_TARGET" -- \
  generate --library "$HOST_TARGET/debug/liblumenna_ffi.dylib" \
  --language swift --out-dir "$GENERATED/bindings"

# Swift imports the C half as the module LumennaCoreFFI (named in crates/surface/uniffi.toml);
# give its map the name Clang looks for in a directory on the import path.
mkdir -p "$GENERATED/include"
cp "$GENERATED/bindings/LumennaCoreFFI.h" "$GENERATED/include/"
cp "$GENERATED/bindings/LumennaCoreFFI.modulemap" "$GENERATED/include/module.modulemap"
