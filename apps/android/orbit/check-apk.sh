#!/bin/bash
# Checks the Orbit app's APK as a person gets it: it says it speaks for itself, so Satellite's
# hook routes it the keys and its launcher lists it, its identifier is one Orbit routes keys to,
# and the core is in it for the devices' ABI.
#
#   apps/android/orbit/check-apk.sh <apk>
set -euo pipefail
APK="$1"
AAPT2="$(ls -d "${ANDROID_HOME:?}"/build-tools/*/ | sort -V | tail -1)aapt2"
MANIFEST="$("$AAPT2" dump xmltree --file AndroidManifest.xml "$APK")"
grep -q 'orbit.self_voicing' <<<"$MANIFEST" || { echo "::error::$APK does not say orbit.self_voicing"; exit 1; }
PACKAGE="$("$AAPT2" dump packagename "$APK")"
case "$PACKAGE" in
  com.orbit.*|*inputmethod*) echo "::error::Orbit does not route keys to $PACKAGE"; exit 1 ;;
esac
LIBS="$(unzip -l "$APK" 'lib/arm64-v8a/*.so' || true)"
grep -q '\.so$' <<<"$LIBS" || { echo "::error::$APK has no core for arm64-v8a"; exit 1; }
echo "$APK: $PACKAGE, self-voicing, core present"
