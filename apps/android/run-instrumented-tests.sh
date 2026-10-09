#!/usr/bin/env bash
# Runs the instrumented tests from APKs already built, on whatever device adb reaches: what
# CI's emulator jobs do with the APKs the build job made, so they need no Rust, NDK or
# Gradle. Screenshots the tests keep land in the directory given, else ./test-output.
#
# Not Gradle's connectedAndroidTest, which also uninstalls the app afterwards: never point
# this at a device whose own Lumenna data matters.
set -euo pipefail

apks=${APKS:-app/build/outputs/apk}
out=${1:-test-output}
package=io.github.emassey0135.lumenna
device_output=/sdcard/Android/media/$package/additional_test_output

adb wait-for-device
# Booted is not ready: the package manager answers a little later, and an install sent
# before it broke off ("Broken pipe"). Then each install is tried a few times.
until adb shell pm path android > /dev/null 2>&1; do sleep 2; done
install() {
  for attempt in 1 2 3; do
    adb install -r -t "$1" && return 0
    echo "Install failed, attempt $attempt; trying again" >&2
    sleep 10
  done
  return 1
}
install "$apks/debug/app-debug.apk"
install "$apks/androidTest/debug/app-debug-androidTest.apk"
adb shell rm -rf "$device_output"
adb shell mkdir -p "$device_output"

# `am instrument` exits 0 whatever the tests did, so its summary decides: "OK (n tests)",
# or "FAILURES!!!" with each failure above it.
result=$(adb shell am instrument -w -e additionalTestOutputDir "$device_output" \
  "$package.test/androidx.test.runner.AndroidJUnitRunner" 2>&1 | tr -d '\r')
echo "$result"

mkdir -p "$out"
adb pull "$device_output/." "$out" > /dev/null 2>&1 || true

grep -q '^OK (' <<< "$result"
