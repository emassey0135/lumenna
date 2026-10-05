#!/bin/bash
# Builds Lumenna's Rust core for Android and regenerates its Kotlin bindings.
#
#   apps/android/build-core.sh <jniLibs dir> <kotlin dir> [debug|release]
#
# Gradle runs this before every build (the app's `buildCore` task). Cargo does nothing when
# nothing changed, so an unchanged core costs a second or two.
#
# Cargo runs in a clean environment, as for the Apple apps: Gradle's environment is the IDE's
# or the shell's, and Cargo's host build scripts should see neither.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
JNI_LIBS="$1"
KOTLIN="$2"
PROFILE="${3:-debug}"
case "$PROFILE" in
  release) PROFILE_FLAG=--release ;;
  *) PROFILE_FLAG= ;;
esac

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
# The newest NDK installed, unless one is named.
NDK="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* 2>/dev/null | sort -V | tail -1)}"
if [ ! -d "$NDK" ]; then
  echo "error: no Android NDK; install one with: sdkmanager \"ndk;<version>\"" >&2
  exit 1
fi

cargo() {
  env -i HOME="$HOME" USER="${USER:-}" \
    PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin" \
    ANDROID_HOME="$SDK" ANDROID_NDK_HOME="$NDK" \
    cargo "$@"
}

cd "$ROOT"
# Arm64 only for now: every phone sold for years, and the emulator on an Apple Silicon Mac.
# The platform is the app's minSdk.
cargo ndk -t arm64-v8a --platform 28 build -p lumenna-ffi --lib $PROFILE_FLAG
# Only the core: `cargo ndk -o` would copy every shared library in the build, Iroh's own
# included, which nothing loads.
mkdir -p "$JNI_LIBS/arm64-v8a"
cp "$ROOT/target/aarch64-linux-android/${PROFILE}/liblumenna_ffi.so" "$JNI_LIBS/arm64-v8a/"

# The bindings come from the interface compiled into a host build of the same crate.
cargo build -p lumenna-ffi --lib
case "$(uname)" in
  Darwin) HOST_LIB="$ROOT/target/debug/liblumenna_ffi.dylib" ;;
  *) HOST_LIB="$ROOT/target/debug/liblumenna_ffi.so" ;;
esac
cargo run -q -p lumenna-ffi --features bindgen --bin uniffi-bindgen -- \
  generate --library "$HOST_LIB" --language kotlin --no-format --out-dir "$KOTLIN"
