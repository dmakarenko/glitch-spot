#!/usr/bin/env bash
# ABOUTME: Builds a debug .app bundle whose productName carries the short commit SHA.
# ABOUTME: Lets you tell dev builds apart in macOS System Settings > Privacy & Security.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

# productName drives the .app folder name, CFBundleName and CFBundleDisplayName at
# once, so the suffix shows wherever macOS names the app. CFBundleIdentifier
# (tauri.conf.json `identifier`) is left untouched, so the rename itself doesn't
# change the app's TCC identity. Adhoc-signed dev builds still re-prompt for
# screen-recording / accessibility per build, since the code signature changes.
CONFIG="$(node scripts/lib/dev-bundle-name.mjs)"
node -e 'const c=JSON.parse(process.argv[1]); console.log(`Debug bundle: ${c.productName}  [${c.identifier}]`)' "$CONFIG"

# Default to the .app only — that's what you install and what shows in Privacy &
# Security. Skip the slow DMG unless the caller selects bundles explicitly.
# Build one never-empty args array so it expands safely under `set -u` on the
# stock macOS Bash 3.2 (an empty-array expansion there aborts the script).
args=(build --debug --config "$CONFIG")
default_bundle=1
for arg in "$@"; do case "$arg" in -b|--bundles) default_bundle=0 ;; esac; done
[ "$default_bundle" -eq 1 ] && args+=(--bundles app)

npm run tauri -- "${args[@]}" "$@"
