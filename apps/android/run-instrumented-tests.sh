#!/usr/bin/env bash
# Runs the instrumented tests from APKs already built, on whatever device adb reaches: what
# CI's emulator jobs do with the APKs the build job made, so they need no Rust, NDK or
# Gradle. Screenshots the tests keep land in the directory given, else ./test-output.
#
# Not Gradle's connectedAndroidTest, which also uninstalls the app afterwards: never point
# this at a device whose own Lumenna data matters.
set -euo pipefail

# $APP is the module: app, the phone's, or wear, the watch's.
app=${APP:-app}
apks=${APKS:-$app/build/outputs/apk}
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
booted() { [ "$(adb shell getprop sys.boot_completed 2> /dev/null | tr -d '\r')" = 1 ]; }
wait_for "the boot to complete" booted
# Booted is not ready: the package manager answers a little later, and an install sent
# before it broke off ("Broken pipe"). Then each install is tried a few times.
wait_for "the package manager" timeout 30 adb shell pm path android

# The device as a person's would be: past its setup screens, awake, and with nothing over the
# home screen. A fresh Wear OS emulator opens on its setup wizard, and on CI the emulator
# action's own screen-timeout setting was sent while adb had lost the device ("device not
# found"), so the screen kept its default. Either way something took the foreground part way
# through the watch's tests, once a run, and the test running then found "No compose
# hierarchies".
adb shell settings put global device_provisioned 1
adb shell settings put secure user_setup_complete 1
adb shell settings put system screen_off_timeout 2147483647
adb shell svc power stayon true
adb shell input keyevent KEYCODE_WAKEUP
adb shell wm dismiss-keyguard > /dev/null 2>&1 || true
# What is in front, as the activity manager says it.
# Read whole, then searched: under pipefail, a grep that stops early fails the pipe.
in_front() {
  local all
  all=$(adb shell dumpsys activity activities 2> /dev/null | tr -d '\r') || return 1
  grep -m1 -E 'topResumedActivity|mResumedActivity' <<< "$all"
}
# Settled is the same ordinary activity in front for three looks in a row, two seconds apart:
# not the setup wizard, not a "not responding" dialog, which a just-booted emulator shows.
settled() {
  local first now windows
  first=$(in_front) || return 1
  if grep -qi -E 'setupwizard|provision' <<< "$first"; then
    adb shell am force-stop "$(sed -E 's/.* ([a-z0-9_.]+)\/.*/\1/' <<< "$first")" > /dev/null 2>&1 || true
    adb shell am start -a android.intent.action.MAIN -c android.intent.category.HOME > /dev/null 2>&1 || true
    return 1
  fi
  windows=$(adb shell dumpsys window 2> /dev/null) || return 1
  if grep -q -E 'mCurrentFocus=.*(Not Responding|Application Error)' <<< "$windows"; then
    adb shell input keyevent KEYCODE_BACK > /dev/null 2>&1 || true
    return 1
  fi
  for _ in 1 2; do
    sleep 2
    now=$(in_front) || return 1
    [ "$now" = "$first" ] || return 1
  done
}
adb shell am start -a android.intent.action.MAIN -c android.intent.category.HOME > /dev/null 2>&1 || true
wait_for "the home screen to settle" settled
install() {
  for attempt in 1 2 3; do
    echo "Installing $1"
    timeout 300 adb install -r -t "$1" && return 0
    echo "Install failed, attempt $attempt; trying again" >&2
    sleep 10
  done
  return 1
}
install "$apks/debug/$app-debug.apk"
install "$apks/androidTest/debug/$app-debug-androidTest.apk"
# Nor is shared storage, where the tests' screenshots go: "Transport endpoint is not
# connected" until it is mounted. The tests make their folder themselves: on CI's fresh
# emulator `/sdcard/Android` is not there yet and the shell may not make it ("Permission
# denied"), where the app may make its own.
wait_for "shared storage" timeout 30 adb shell ls /sdcard/
adb shell rm -rf "$device_output/*" || true

# `am instrument` exits 0 whatever the tests did, so its summary decides: "OK (n tests)",
# or "FAILURES!!!" with each failure above it. $TESTS narrows it to some, as the
# instrumentation's `class` argument takes them: Class#method, comma-separated.
only=()
[ -n "${TESTS:-}" ] && only=(-e class "$TESTS")
echo "Running the tests"
result=$(adb shell am instrument -w ${only[@]+"${only[@]}"} -e additionalTestOutputDir "$device_output" \
  "$package.test/androidx.test.runner.AndroidJUnitRunner" 2>&1 | tr -d '\r') || true
echo "$result"

mkdir -p "$out"
adb pull "$device_output/." "$out" > /dev/null 2>&1 || true

if ! grep -q '^OK (' <<< "$result"; then
  # What crashed, if anything did: the summary only says "Process crashed".
  echo "--- crash log"
  adb logcat -d -b crash | tail -60 || true
  # What came to the front, and what stopped answering: what took a test's screen away.
  echo "--- activities resumed, and anything not responding"
  adb logcat -d -b events | grep -E 'wm_set_resumed_activity|am_anr|wm_on_paused_called' | tail -40 || true
  exit 1
fi
