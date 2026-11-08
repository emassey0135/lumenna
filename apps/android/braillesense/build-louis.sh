#!/bin/bash
# Builds liblouis for the BrailleSense and gathers its tables: the service takes Braille ASCII or
# dots, so the app translates in the person's own table itself, as HIMS' apps do.
#
#   apps/android/braillesense/build-louis.sh <jniLibs dir> <assets dir> [debug|release]
#
# Gradle runs this before every build (`buildLouis`); what is built already is left. The source
# is the release's own tarball, checked against its hash, in Gradle's build directory.
set -euo pipefail

VERSION=3.39.0
SHA256=629fa8cb0dfd9ad457c5bf47a42f0953b673e62c8ad6b1d03ddc4e2bd20008f1
HERE="$(cd "$(dirname "$0")" && pwd)"
JNI_LIBS="$1"
ASSETS="$2"
PROFILE="${3:-debug}"
WORK="$HERE/build/louis"
SOURCE="$WORK/liblouis-$VERSION"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
NDK="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* 2>/dev/null | sort -V | tail -1)}"
if [ ! -d "$NDK" ]; then
  echo "error: no Android NDK; install one with: sdkmanager \"ndk;<version>\"" >&2
  exit 1
fi
TOOLCHAIN="$(ls -d "$NDK"/toolchains/llvm/prebuilt/* | head -1)/bin"

mkdir -p "$WORK"
if [ ! -d "$SOURCE" ]; then
  tarball="$WORK/liblouis-$VERSION.tar.gz"
  [ -f "$tarball" ] || curl -fsSL -o "$tarball" \
    "https://github.com/liblouis/liblouis/releases/download/v$VERSION/liblouis-$VERSION.tar.gz"
  # shasum on the Mac, sha256sum on CI's Linux.
  if command -v sha256sum >/dev/null; then echo "$SHA256  $tarball" | sha256sum -c - >/dev/null; else echo "$SHA256  $tarball" | shasum -a 256 -c - >/dev/null; fi
  tar -xzf "$tarball" -C "$WORK"
fi

# What the configure script would have written, for this one target: wide characters are UTF-16,
# as JNI's are, and the tables are found through lou_setDataPath alone.
GENERATED="$WORK/generated"
mkdir -p "$GENERATED"
sed 's/@WIDECHAR_TYPE@/unsigned short int/' "$SOURCE/liblouis/liblouis.h.in" > "$GENERATED/liblouis.h"
cat > "$GENERATED/config.h" <<EOF
#define PACKAGE_VERSION "$VERSION"
#define TABLESDIR ""
EOF

# Only the BrailleSense runs this, and every BrailleSense is arm64.
ABI=arm64-v8a
OUT="$JNI_LIBS/$ABI"
mkdir -p "$OUT"
LIB="$OUT/liblumenna-louis.so"
if [ ! -f "$LIB" ] || [ "$HERE/native/louis.c" -nt "$LIB" ]; then
  case "$PROFILE" in
    release) OPTIMIZE="-O2 -DNDEBUG" ;;
    *) OPTIMIZE="-O1" ;;
  esac
  # 16 KB aligned, as every library of the apps is.
  "$TOOLCHAIN/aarch64-linux-android28-clang" -shared -fPIC $OPTIMIZE -w \
    -I"$GENERATED" -I"$SOURCE/liblouis" \
    -Wl,-z,max-page-size=16384 -Wl,--gc-sections -ffunction-sections -fdata-sections \
    "$SOURCE"/liblouis/{commonTranslationFunctions,compileTranslationTable,logging,lou_backTranslateString,lou_translateString,maketable,metadata,pattern,utils}.c \
    "$HERE/native/louis.c" \
    -o "$LIB"
  "$TOOLCHAIN/llvm-strip" --strip-unneeded "$LIB"
fi

TABLES="$ASSETS/louis/liblouis/tables"
if [ ! -f "$TABLES/.version-$VERSION" ]; then
  rm -rf "$ASSETS/louis"
  mkdir -p "$TABLES"
  find "$SOURCE/tables" -maxdepth 1 -type f \( -name '*.ctb' -o -name '*.utb' -o -name '*.cti' -o -name '*.uti' -o -name '*.tbl' -o -name '*.dis' -o -name '*.dic' \) -exec cp {} "$TABLES/" \;
  touch "$TABLES/.version-$VERSION"
fi
