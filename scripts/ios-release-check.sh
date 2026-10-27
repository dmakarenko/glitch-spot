#!/usr/bin/env bash
# ABOUTME: Builds the Example app in Release and Debug and checks that only the Debug binaries contain the SDK.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
example="$root/packages/ios/Example"
marker="Annotate in Glitch Spot"
# A new folder per run, so a result never comes from a binary that an earlier run built.
out="$root/.build/ios-release-check/$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p "$out"

cd "$root"
"$(mise where xcodegen)/bin/xcodegen" generate --quiet --spec "$example/project.yml"

# Prints "CLEAN" or "CONTAINS SDK" for one configuration; exits the script if the build or the binary is missing.
check() {
  local config="$1" log="$out/$1.log"
  if ! xcodebuild build \
    -project "$example/GlitchSpotExample.xcodeproj" \
    -scheme GlitchSpotExample \
    -configuration "$config" \
    -sdk iphonesimulator \
    -destination "generic/platform=iOS Simulator" \
    -derivedDataPath "$out/DerivedData" \
    CODE_SIGNING_ALLOWED=NO >"$log" 2>&1; then
    tail -20 "$log" >&2
    echo "ios-release-check: the $config build failed, full log: $log" >&2
    exit 1
  fi

  local app="$out/DerivedData/Build/Products/$config-iphonesimulator/GlitchSpotExample.app"
  if [ ! -f "$app/GlitchSpotExample" ]; then
    echo "ios-release-check: the $config build made no app binary at $app" >&2
    exit 1
  fi

  # Debug builds can put the app code in GlitchSpotExample.debug.dylib, so search every Mach-O file in the app.
  local binaries=0 found=0 file
  while IFS= read -r -d '' file; do
    file -b "$file" | grep -q '^Mach-O' || continue
    binaries=$((binaries + 1))
    if strings -a "$file" | grep -qF "$marker"; then found=1; fi
  done < <(find "$app" -type f -print0)
  if [ "$binaries" -eq 0 ]; then
    echo "ios-release-check: no Mach-O binary in $app" >&2
    exit 1
  fi
  [ "$found" -eq 1 ] && echo "CONTAINS SDK" || echo "CLEAN"
}

release="$(check Release)"
debug="$(check Debug)"
echo "Release: $release"
echo "Debug: $debug"
[ "$release" = "CLEAN" ] && [ "$debug" = "CONTAINS SDK" ]
