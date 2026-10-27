#!/usr/bin/env bash
# ABOUTME: Runs the hosted GlitchSpotKit tests in the Example app on an iOS Simulator and prints each test result.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
example="$root/packages/ios/Example"
out="$root/.build/ios-test"
destination="platform=iOS Simulator,name=${GLITCH_SPOT_SIM:-iPhone 17 Pro},OS=${GLITCH_SPOT_SIM_OS:-26.5}"
result="$out/$(date +%Y%m%d-%H%M%S).xcresult"
log="$out/xcodebuild.log"
mkdir -p "$out"

cd "$root"
"$(mise where xcodegen)/bin/xcodegen" generate --quiet --spec "$example/project.yml"

status=0
xcodebuild test \
  -project "$example/GlitchSpotExample.xcodeproj" \
  -scheme GlitchSpotExample \
  -destination "$destination" \
  -derivedDataPath "$out/DerivedData" \
  -resultBundlePath "$result" >"$log" 2>&1 || status=$?

if [ ! -d "$result" ]; then
  tail -40 "$log"
  echo "ios-test: xcodebuild made no result bundle (exit $status), full log: $log" >&2
  exit 1
fi

xcrun xcresulttool get test-results tests --path "$result" |
  jq -r '.. | objects | select(.nodeType? == "Test Case") | "\(.result)  \(.nodeIdentifier // .name)"'
xcrun xcresulttool get test-results summary --path "$result" |
  jq -r '"passed \(.passedTests), failed \(.failedTests), skipped \(.skippedTests)"'
if [ "$status" -ne 0 ]; then
  rg -n 'error:' "$log" | head -20 || true
  echo "ios-test: xcodebuild exit $status, full log: $log" >&2
fi
exit "$status"
