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

# --- Build (unsigned, arm64) ---
echo "Building DMG…"
npm run tauri -- build

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
This build is **unsigned**. On first launch macOS will block it.

Right-click **Glitch Spot.app** → **Open** (then confirm in the dialog), or run:

```
xattr -dr com.apple.quarantine "/Applications/Glitch Spot.app"
```

---
NOTE
gh release create "$TAG" "$ARTIFACT" --title "$TAG" --generate-notes --notes-file "$BODY"
rm -f "$BODY"
echo "Released $TAG ✅"
