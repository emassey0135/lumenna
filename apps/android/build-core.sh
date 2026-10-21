#!/bin/bash
# Builds Lumenna's Rust core for Android — the app and the device's store, through BoltFFI
# (crates/ffi) — and regenerates its Kotlin.
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
# LUMENNA_CORE_PROFILE overrides the variant's: CI builds the core optimized even in a debug
# APK, which is a fraction of the size, quicker to install on an emulator, and nearer what ships.
PROFILE="${LUMENNA_CORE_PROFILE:-${3:-debug}}"
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

# arm64 for phones and watches, x86_64 for ChromeOS and the emulators on Intel machines and
# CI, and 32-bit ARM for the many Wear OS watches whose Android is 32-bit; the phone's APK
# leaves that one out (abiFilters). LUMENNA_ANDROID_ABIS narrows it for a quicker build.
ABIS="${LUMENNA_ANDROID_ABIS:-arm64-v8a x86_64 armeabi-v7a}"

# The app through BoltFFI (crates/ffi): it builds the library per ABI, compiles its JNI glue
# into it, and writes its Kotlin; an overlay points its outputs into this build's directories.
BOLT="$(dirname "$KOTLIN")/boltffi"
architectures=""
for abi in $ABIS; do
  case "$abi" in
    arm64-v8a) architectures="$architectures\"arm64\", " ;;
    x86_64) architectures="$architectures\"x86_64\", " ;;
    armeabi-v7a) architectures="$architectures\"armv7\", " ;;
  esac
done
# Its Kotlin afresh each time: a file it no longer writes would otherwise stay and redeclare.
rm -rf "$BOLT/kotlin"
mkdir -p "$BOLT"
cat > "$BOLT/overlay.toml" <<EOF
[targets.android]
output = "$BOLT"
architectures = [${architectures%, }]

[targets.android.pack]
output = "$JNI_LIBS"

[targets.android.kotlin]
output = "$BOLT/kotlin"
EOF
clean() {
  env -i HOME="$HOME" USER="${USER:-}" ${CARGO_HOME:+CARGO_HOME="$CARGO_HOME"} ${RUSTUP_HOME:+RUSTUP_HOME="$RUSTUP_HOME"} \
    PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin" \
    ANDROID_HOME="$SDK" ANDROID_NDK_HOME="$NDK" "$@"
}
# Each ABI's static library built here, alone, and BoltFFI only links it into the JNI library
# and writes the Kotlin (--no-build): crates/ffi is an rlib too, for the Rust apps, and rustc
# builds an artifact made beside an rlib without whole-program optimization. The C the core's
# dependencies compile (SQLite, ring, BLAKE3) takes the NDK's clang at the apps' minimum API,
# as BoltFFI's own build gives it (crates/ffi/boltffi.toml's min_sdk).
MIN_SDK=28
BIN="$(ls -d "$NDK"/toolchains/llvm/prebuilt/*/bin | head -1)"
for abi in $ABIS; do
  case "$abi" in
    arm64-v8a) triple=aarch64-linux-android; clang=aarch64-linux-android ;;
    x86_64) triple=x86_64-linux-android; clang=x86_64-linux-android ;;
    armeabi-v7a) triple=armv7-linux-androideabi; clang=armv7a-linux-androideabi ;;
    *) continue ;;
  esac
  lower="${triple//-/_}"
  upper="$(echo "$lower" | tr '[:lower:]' '[:upper:]')"
  (cd "$ROOT" && clean env "CC_$lower=$BIN/$clang$MIN_SDK-clang" "AR_$lower=$BIN/llvm-ar" \
    "CARGO_TARGET_${upper}_LINKER=$BIN/$clang$MIN_SDK-clang" \
    cargo rustc -p lumenna-ffi --lib --crate-type staticlib $PROFILE_FLAG --target "$triple" --quiet)
done
(cd "$ROOT/crates/ffi" && clean boltffi pack android $PROFILE_FLAG --no-build --quiet --overlay "$BOLT/overlay.toml")
# Only the Kotlin: the JNI glue beside it is compiled into the library already. What an
# earlier build wrote there goes first, the library of before the crate was renamed included.
rm -rf "$KOTLIN/io" "$JNI_LIBS"/*/liblumenna-mobile.so
mkdir -p "$KOTLIN/io/github/emassey0135/lumenna"
cp -R "$BOLT/kotlin/io/github/emassey0135/lumenna/app" "$KOTLIN/io/github/emassey0135/lumenna/"


# A release core without its symbols and debug information: BoltFFI links the library itself,
# with the JNI glue, so Cargo's `strip` never reaches it, and the Gradle plugin strips only with
# the one NDK version it expects, copying the library whole when that is not installed (35 MB
# for arm64, where stripped it is a fraction of that). The NDK that built it strips it.
if [ "$PROFILE" = release ]; then
  STRIP="$(ls "$NDK"/toolchains/llvm/prebuilt/*/bin/llvm-strip | head -1)"
  for library in "$JNI_LIBS"/*/liblumenna-ffi.so; do
    "$STRIP" --strip-all "$library"
  done
fi
