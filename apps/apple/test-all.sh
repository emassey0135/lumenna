#!/usr/bin/env bash
# Every iOS UI test on a Pro-sized iPhone, an iPhone SE and an iPad at once, built once:
# what CI runs as three jobs, here as one xcodebuild. Each device's tests also share out
# over clones of it. Diagnostics collection is off: on a failure it held xcodebuild for ten
# minutes before reporting. A test is stopped after three minutes: one waiting forever held
# a whole run. Results go to $RESULTS (default /tmp/lumenna-ios.xcresult).
#
#   apps/apple/test-all.sh                 everything
#   apps/apple/test-all.sh -only-testing:LumennaUITests/SidebarUITests
set -euo pipefail
cd "$(dirname "$0")"
[ -d Lumenna.xcodeproj ] || xcodegen

newest() { xcrun simctl list devices available | grep -E "$1" | tail -1 | grep -oE '[0-9A-F]{8}-([0-9A-F]{4}-){3}[0-9A-F]{12}'; }
iphone=$(newest '^\s+iPhone [0-9]+ Pro \(')
ipad=$(newest '^\s+iPad Pro 11-inch')
# Xcode lists no SE of its own, but its iOS still runs on one: made once, kept.
se=$(newest '^\s+Lumenna iPhone SE \(' || true)
if [ -z "$se" ]; then
  runtime=$(xcrun simctl list runtimes available | grep -oE 'com\.apple\.CoreSimulator\.SimRuntime\.iOS-[0-9-]+' | tail -1)
  se=$(xcrun simctl create "Lumenna iPhone SE" com.apple.CoreSimulator.SimDeviceType.iPhone-SE-3rd-generation "$runtime")
fi

results=${RESULTS:-/tmp/lumenna-ios.xcresult}
rm -rf "$results"
xcodebuild -project Lumenna.xcodeproj -scheme Lumenna \
  -destination "platform=iOS Simulator,id=$iphone" \
  -destination "platform=iOS Simulator,id=$se" \
  -destination "platform=iOS Simulator,id=$ipad" \
  -maximum-concurrent-test-simulator-destinations 3 \
  -parallel-testing-enabled YES -parallel-testing-worker-count "${WORKERS:-2}" \
  -collect-test-diagnostics never -resultBundlePath "$results" \
  -test-timeouts-enabled YES -default-test-execution-time-allowance 180 -maximum-test-execution-time-allowance 300 \
  test "$@"
