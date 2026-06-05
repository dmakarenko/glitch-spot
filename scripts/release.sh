#!/usr/bin/env bash
# ABOUTME: One-command local production release for Glitch Spot.
# ABOUTME: Bumps version, builds an unsigned arm64 DMG, publishes to GitHub Releases.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

BUMP=""
DRY=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY=1 ;;
    patch|minor|major) BUMP="$arg" ;;
    [0-9]*.[0-9]*.[0-9]*) BUMP="$arg" ;;
    *) echo "Unknown argument: $arg" >&2; exit 2 ;;
  esac
done
if [ -z "$BUMP" ]; then
  echo "Usage: npm run release -- [patch|minor|major|X.Y.Z] [--dry-run]" >&2
  exit 2
fi

# --- Pre-flight ---
need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing required tool: $1" >&2; exit 1; }; }
need node; need gh; need cargo; need git

# This pipeline produces an arm64-only DMG (labeled _aarch64). Building on a non-arm64
# host would mislabel the artifact, so refuse rather than ship a wrong-arch DMG.
[ "$(uname -m)" = "arm64" ] || { echo "This release builds an arm64-only DMG; host arch is '$(uname -m)'. Aborting." >&2; exit 1; }

# The bundle identifier anchors both TCC permission grants and the signing designated
# requirement. Renaming it orphans every existing grant with no migration path, so pin it.
EXPECTED_ID="spot.glitch.release"
ACTUAL_ID="$(node -p "require('$ROOT/src-tauri/tauri.conf.json').identifier")"
[ "$ACTUAL_ID" = "$EXPECTED_ID" ] || { echo "tauri.conf.json identifier is '$ACTUAL_ID', expected '$EXPECTED_ID'. Renaming the bundle id orphans all TCC grants — aborting." >&2; exit 1; }

if [ "$DRY" -eq 0 ]; then
  branch="$(git rev-parse --abbrev-ref HEAD)"
  [ "$branch" = "main" ] || { echo "Releases must be cut from 'main' (currently on '$branch'). Use --dry-run to test elsewhere." >&2; exit 1; }
  [ -z "$(git status --porcelain)" ] || { echo "Working tree is not clean. Commit or stash first." >&2; exit 1; }
  gh auth status >/dev/null 2>&1 || { echo "gh is not authenticated. Run: gh auth login" >&2; exit 1; }
fi

CURRENT="$(node scripts/lib/version.mjs current)"
NEW="$(node scripts/lib/version.mjs next "$BUMP")"
TAG="v$NEW"
echo "Release: $CURRENT -> $NEW (tag $TAG)$([ "$DRY" -eq 1 ] && echo ' [dry-run]')"

if [ "$DRY" -eq 0 ]; then
  if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "Tag $TAG already exists locally." >&2; exit 1
  fi
  # Also check the remote so we don't build/commit/push only to fail pushing a tag
  # that exists on origin but wasn't fetched locally.
  if git ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null 2>&1; then
    echo "Tag $TAG already exists on origin." >&2; exit 1
  fi
fi

# --- Version ---
# Snapshot the version files first so a dry run — or any failure before the release
# commit — restores exactly the bump, preserving any unrelated uncommitted edits
# (which `git checkout --` would have discarded).
VERSION_FILES=(package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock)
SNAP_DIR="$(mktemp -d)"
COMMITTED=0
restore_versions() { for f in "${VERSION_FILES[@]}"; do cp "$SNAP_DIR/$f" "$f"; done; }
cleanup() {
  local rc=$?
  if [ "$rc" -ne 0 ] && [ "$COMMITTED" -eq 0 ]; then
    echo "Release failed before commit — restoring version files." >&2
    restore_versions
  fi
  rm -rf "$SNAP_DIR"
}
trap cleanup EXIT
for f in "${VERSION_FILES[@]}"; do
  mkdir -p "$SNAP_DIR/$(dirname "$f")"
  cp "$f" "$SNAP_DIR/$f"
done

node scripts/lib/version.mjs apply "$BUMP" >/dev/null

# --- Build (signed with the stable release identity, arm64) ---
# Sign releases with a self-signed code-signing identity so the designated requirement
# is anchored to the certificate rather than a per-build cdhash. That keeps the app's
# identity stable across rebuilds, so a granted Screen Recording permission survives
# updates instead of re-prompting every release. The identity lives only on this
# machine — keep the exported .p12 backed up. Dev/clone builds stay ad-hoc via
# tauri.conf.json's signingIdentity "-".
#
# The cert is self-signed (CSSMERR_TP_NOT_TRUSTED), so it never appears under
# `find-identity -v` (valid-only); list without -v and match by name.
SIGN_ID="Glitch Spot Self-Signed"
if security find-identity -p codesigning 2>/dev/null | grep -q "$SIGN_ID"; then
  export APPLE_SIGNING_IDENTITY="$SIGN_ID"
  echo "Signing identity: $SIGN_ID"
elif [ "$DRY" -eq 1 ]; then
  echo "WARNING: signing identity '$SIGN_ID' not in keychain — dry-run will build ad-hoc." >&2
else
  echo "Release signing identity '$SIGN_ID' not found. Import the .p12 backup into the login keychain first." >&2
  exit 1
fi

echo "Building DMG…"
npm run tauri -- build

# Without a valid bundle signature, Apple Silicon reports downloaded builds as "damaged
# and can't be opened". Refuse to ship a DMG whose app fails strict signature verification.
APP="$(ls -dt src-tauri/target/release/bundle/macos/*.app 2>/dev/null | head -1 || true)"
[ -n "$APP" ] || { echo "No .app bundle was produced under target/release/bundle/macos/." >&2; exit 1; }
codesign --verify --deep --strict --verbose=2 "$APP" || { echo "Code signature verification failed for $APP — refusing to ship." >&2; exit 1; }

# When signed with the release identity, the designated requirement must be anchored to
# the certificate (stable across rebuilds), not a bare cdhash. A cdhash DR means the cert
# was not actually used — every update would then orphan the prior permission grant.
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  codesign -d -r- "$APP" 2>&1 | grep -q "certificate leaf" || { echo "Signed bundle's designated requirement is not certificate-anchored — refusing to ship." >&2; exit 1; }
fi

DMG="$(ls -t src-tauri/target/release/bundle/dmg/*.dmg 2>/dev/null | head -1 || true)"
[ -n "$DMG" ] || { echo "No DMG was produced under target/release/bundle/dmg/." >&2; exit 1; }
ARTIFACT="/tmp/Glitch-Spot_${NEW}_aarch64.dmg"
cp "$DMG" "$ARTIFACT"
echo "Artifact: $ARTIFACT"

if [ "$DRY" -eq 1 ]; then
  echo "[dry-run] Reverting version changes."
  restore_versions
  echo "[dry-run] Would commit, tag $TAG, push, create the release, and upload $ARTIFACT."
  exit 0
fi

# --- Commit + tag ---
git commit -aqm "chore(release): $TAG"
COMMITTED=1
git tag "$TAG"

# --- Publish ---
git push origin "$(git rev-parse --abbrev-ref HEAD)"
git push origin "$TAG"

BODY="$(mktemp)"
cat > "$BODY" <<'NOTE'
This build is **signed but not notarized by Apple**, so macOS shows *"Apple could not verify…"* — first when you open the downloaded **`.dmg`**, and possibly again on the app's first launch. This is expected; the app is safe to open.

**Easiest — clear the quarantine flag in Terminal, then open the DMG:**

```
xattr -dr com.apple.quarantine ~/Downloads/Glitch-Spot_*_aarch64.dmg
open ~/Downloads/Glitch-Spot_*_aarch64.dmg
```

Drag **Glitch Spot** to Applications. If the app is still blocked on first launch:

```
xattr -dr com.apple.quarantine "/Applications/Glitch Spot.app"
```

**Or via System Settings** (repeat at each prompt — once for the DMG, once for the app):

1. Click **Done** (not "Move to Bin").
2. Open **System Settings → Privacy & Security**, scroll to **Security**, and click **Open Anyway**.

---
NOTE
gh release create "$TAG" "$ARTIFACT" --title "$TAG" --generate-notes --notes-file "$BODY"
rm -f "$BODY"
echo "Released $TAG ✅"
