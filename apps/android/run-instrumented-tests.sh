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

# macOS has no `timeout`; there each try just runs to its end.
command -v timeout > /dev/null || timeout() { shift; "$@"; }

# Waits until a command succeeds, saying what for, and gives up after five minutes: a wait
# that never ends showed nothing in CI until the runner was shut down.
wait_for() {
  local what=$1; shift
  echo "Waiting for $what"
  for _ in $(seq 150); do
    "$@" > /dev/null 2>&1 && return 0
    sleep 2
  done
  echo "Gave up waiting for $what; the last try said:" >&2
  "$@" >&2 || true
  exit 1
}

wait_for "the device" timeout 60 adb wait-for-device
# Booted is not ready: the package manager answers a little later, and an install sent
# before it broke off ("Broken pipe"). Then each install is tried a few times.
wait_for "the package manager" timeout 30 adb shell pm path android
# Nor is shared storage, where the tests' screenshots go: "Transport endpoint is not
# connected" until it is mounted, even after /sdcard answers. Ready is when the folder can
# be made.
wait_for "shared storage" timeout 30 adb shell mkdir -p "$device_output"
install() {
  for attempt in 1 2 3; do
    echo "Installing $1"
    timeout 300 adb install -r -t "$1" && return 0
    echo "Install failed, attempt $attempt; trying again" >&2
    sleep 10
  done
  return 1
}
install "$apks/debug/app-debug.apk"
install "$apks/androidTest/debug/app-debug-androidTest.apk"
adb shell rm -rf "$device_output/*" || true

# `am instrument` exits 0 whatever the tests did, so its summary decides: "OK (n tests)",
# or "FAILURES!!!" with each failure above it. $TESTS narrows it to some, as the
# instrumentation's `class` argument takes them: Class#method, comma-separated.
only=()
[ -n "${TESTS:-}" ] && only=(-e class "$TESTS")
echo "Running the tests"
result=$(adb shell am instrument -w "${only[@]}" -e additionalTestOutputDir "$device_output" \
  "$package.test/androidx.test.runner.AndroidJUnitRunner" 2>&1 | tr -d '\r')
echo "$result"

mkdir -p "$out"
adb pull "$device_output/." "$out" > /dev/null 2>&1 || true

if ! grep -q '^OK (' <<< "$result"; then
  # What crashed, if anything did: the summary only says "Process crashed".
  echo "--- crash log"
  adb logcat -d -b crash | tail -60 || true
  exit 1
fi
