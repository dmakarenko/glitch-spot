# Production Builds → GitHub Releases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A single command (`npm run release -- minor`) that bumps the version, builds an unsigned arm64 macOS DMG locally, and publishes it to GitHub Releases. (Originally specified universal — see the Revision note below for why it became arm64-only.)

**Architecture:** A bash orchestrator (`scripts/release.sh`) handles pre-flight, git, the Tauri build, and `gh` publishing. All version-math and version-file editing — the only logic worth testing — lives in a node module (`scripts/lib/version.mjs`) covered by the project's existing Vitest runner. The bash layer is verified end-to-end via a `--dry-run` that builds the DMG but makes no git/GitHub changes.

**Tech Stack:** bash, node (v26, ESM), Vitest, `@tauri-apps/cli` (tauri build), `gh` CLI, `jq`.

**Design seed:** `docs/seeds/2026-06-01-production-builds-seed.md`

---

## Revision (2026-06-02): built arm64-only, not universal

Tasks 1–3 were implemented as written. **Task 4 changed during execution:** the universal
build failed at bundling because the crate's second binary (`mcp_server`, eval-only) is
auto-bundled by Tauri but not lipo-merged for universal targets; a `required-features` gate
did not help (Tauri enumerates `[[bin]]` from cargo metadata regardless). Per Dmitry's
revised decision, the pipeline builds **arm64-only**: `release.sh` runs `npm run tauri -- build`
(no `--target universal-apple-darwin`, no x86_64 target install), reads the DMG from
`src-tauri/target/release/bundle/dmg/`, and names the artifact `Glitch-Spot_X.Y.Z_aarch64.dmg`.
The shipped `.app` includes the small unused `mcp_server`. Verified via `--dry-run`
(16MB DMG produced, tree clean). The Task 4 script blocks below show the original universal
form; the committed `scripts/release.sh` is the arm64 version. See the seed's Revision note.

## Context the worker needs

- **Repo:** `dmakarenko/glitch-spot` (private). Releases go to its GitHub Releases.
- **Four files carry a version**, and they currently disagree — the release must unify them:
  - `package.json` → `"version": "0.0.0"` (drifted; gets corrected on first release)
  - `src-tauri/tauri.conf.json` → `"version": "0.1.0"` — **source of truth for the current version**
  - `src-tauri/Cargo.toml` → `version = "0.1.0"` (line 5, inside `[package]`)
  - `src-tauri/Cargo.lock` → the `glitch_spot` entry: `name = "glitch_spot"` immediately followed by `version = "0.1.0"`
- **`cargo set-version` is NOT installed** (no cargo-edit) — do not use it. The node helper edits `Cargo.toml`/`Cargo.lock` by text, which is why those edits are unit-tested.
- **Vitest only scans `src/**`** (see `vitest.config.ts` `include`). Task 1 extends `include` so `scripts/**/*.test.ts` runs.
- **`package.json` has `"type": "module"`** — `.mjs` and `.ts` are ESM.
- Tauri build convention in this repo needs cargo on PATH: `release.sh` exports `PATH="$HOME/.cargo/bin:$PATH"`.
- The Tauri build runs `beforeBuildCommand` (`npm run build` = `tsc && vite build`) automatically, so the frontend is built as part of `tauri build`.
- `gh release create --generate-notes --notes-file <f>` **prepends** the file's contents above GitHub's auto-generated notes — confirmed via `gh release create --help`.

## File structure

| File | Responsibility |
|------|----------------|
| `scripts/lib/version.mjs` (create) | Pure version transforms (`nextVersion`, per-file setters), `applyVersion` writer, and a small CLI (`current` / `next <bump>` / `apply <bump>`). |
| `scripts/lib/version.test.ts` (create) | Vitest unit tests for all of the above. |
| `vitest.config.ts` (modify) | Add `scripts/**/*.test.ts` to `include`. |
| `scripts/release.sh` (create) | Bash orchestrator: pre-flight → version → build → commit/tag → publish; `--dry-run` short-circuits before any mutation. |
| `package.json` (modify) | Add `"release": "bash scripts/release.sh"` script. |
| `README.md` (create) | Install note (unsigned-app Gatekeeper steps) + how to cut a release. |

---

## Task 1: Version compute + test wiring

**Files:**
- Create: `scripts/lib/version.mjs`
- Test: `scripts/lib/version.test.ts`
- Modify: `vitest.config.ts`

- [ ] **Step 1: Extend Vitest to scan scripts**

Modify `vitest.config.ts` — change the `include` array:

```ts
    include: ["src/**/*.test.ts", "src/**/*.test.tsx", "scripts/**/*.test.ts"],
```

- [ ] **Step 2: Write the failing test for `nextVersion`**

Create `scripts/lib/version.test.ts`:

```ts
// ABOUTME: Unit tests for the release version helper (compute + multi-file sync).
// ABOUTME: Runs under the project's Vitest config (scripts/**/*.test.ts).
import { describe, it, expect } from "vitest";
import { nextVersion } from "./version.mjs";

describe("nextVersion", () => {
  it("bumps patch, minor, major", () => {
    expect(nextVersion("0.1.0", "patch")).toBe("0.1.1");
    expect(nextVersion("0.1.0", "minor")).toBe("0.2.0");
    expect(nextVersion("0.1.9", "major")).toBe("1.0.0");
  });
  it("accepts an explicit X.Y.Z", () => {
    expect(nextVersion("0.1.0", "2.3.4")).toBe("2.3.4");
  });
  it("rejects an invalid bump or current version", () => {
    expect(() => nextVersion("0.1.0", "huge")).toThrow();
    expect(() => nextVersion("not-a-version", "patch")).toThrow();
  });
});
```

- [ ] **Step 3: Run the test, verify it fails**

Run: `npm test -- scripts/lib/version.test.ts`
Expected: FAIL — cannot resolve `./version.mjs` (module does not exist yet).

- [ ] **Step 4: Implement `nextVersion` in a new module**

Create `scripts/lib/version.mjs`:

```js
// ABOUTME: Computes the next semver and writes it across all version-bearing files.
// ABOUTME: Pure transforms + a small CLI used by scripts/release.sh.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const FILES = {
  packageJson: "package.json",
  tauriConf: "src-tauri/tauri.conf.json",
  cargoToml: "src-tauri/Cargo.toml",
  cargoLock: "src-tauri/Cargo.lock",
};

export function nextVersion(current, bump) {
  if (/^\d+\.\d+\.\d+$/.test(bump)) return bump;
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
  if (!m) throw new Error(`Invalid current version: ${current}`);
  const [major, minor, patch] = m.slice(1).map(Number);
  if (bump === "major") return `${major + 1}.0.0`;
  if (bump === "minor") return `${major}.${minor + 1}.0`;
  if (bump === "patch") return `${major}.${minor}.${patch + 1}`;
  throw new Error(`Invalid bump: ${bump} (expected patch|minor|major|X.Y.Z)`);
}
```

- [ ] **Step 5: Run the test, verify it passes**

Run: `npm test -- scripts/lib/version.test.ts`
Expected: PASS — the `nextVersion` describe-block is green (the test file imports only `nextVersion` at this point).

- [ ] **Step 6: Commit**

```bash
git add scripts/lib/version.mjs scripts/lib/version.test.ts vitest.config.ts
git commit -m "feat(release): version compute helper + scripts test wiring"
```

---

## Task 2: Per-file version setters (pure string transforms)

**Files:**
- Modify: `scripts/lib/version.mjs`
- Test: `scripts/lib/version.test.ts`

- [ ] **Step 1: Grow the import, then write failing tests for the three setters**

First, update the `./version.mjs` import line at the top of `scripts/lib/version.test.ts` to add the setters:

```ts
import {
  nextVersion,
  setJsonVersion,
  setCargoTomlVersion,
  setCargoLockVersion,
} from "./version.mjs";
```

Then append to `scripts/lib/version.test.ts` (after the `nextVersion` describe):

```ts
describe("setJsonVersion", () => {
  it("changes only the version field", () => {
    const src = '{\n  "name": "x",\n  "version": "0.0.0",\n  "scripts": {}\n}\n';
    const out = setJsonVersion(src, "1.2.3");
    expect(JSON.parse(out).version).toBe("1.2.3");
    expect(out).toContain('"name": "x"');
  });
  it("throws when no version field exists", () => {
    expect(() => setJsonVersion('{\n  "name": "x"\n}\n', "1.2.3")).toThrow();
  });
});

describe("setCargoTomlVersion", () => {
  it("changes the package version, not dependency versions", () => {
    const src =
      '[package]\nname = "glitch_spot"\nversion = "0.1.0"\n\n' +
      '[dependencies]\nfoo = { version = "9.9.9" }\n';
    const out = setCargoTomlVersion(src, "1.2.3");
    expect(out).toContain('version = "1.2.3"');
    expect(out).toContain('foo = { version = "9.9.9" }');
  });
});

describe("setCargoLockVersion", () => {
  it("targets the named package entry only", () => {
    const src =
      'name = "other"\nversion = "5.0.0"\n\n' +
      'name = "glitch_spot"\nversion = "0.1.0"\ndependencies = [\n';
    const out = setCargoLockVersion(src, "1.2.3");
    expect(out).toContain('name = "glitch_spot"\nversion = "1.2.3"');
    expect(out).toContain('name = "other"\nversion = "5.0.0"');
  });
});
```

- [ ] **Step 2: Run, verify failure**

Run: `npm test -- scripts/lib/version.test.ts`
Expected: FAIL — `setJsonVersion`/`setCargoTomlVersion`/`setCargoLockVersion` are not exported.

- [ ] **Step 3: Implement the three setters**

Add to `scripts/lib/version.mjs` (after `nextVersion`):

```js
export function setJsonVersion(text, version) {
  const out = text.replace(/("version"\s*:\s*)"[^"]*"/, `$1"${version}"`);
  if (out === text) throw new Error('No "version" field found in JSON');
  return out;
}

export function setCargoTomlVersion(text, version) {
  // The first line-anchored `version = "..."` is the [package] version;
  // dependency versions are inline (`foo = { version = ... }`) or in later tables.
  const out = text.replace(/^version = "[^"]*"$/m, `version = "${version}"`);
  if (out === text) throw new Error("No package version found in Cargo.toml");
  return out;
}

export function setCargoLockVersion(text, version, pkg = "glitch_spot") {
  const re = new RegExp(`(name = "${pkg}"\\nversion = )"[^"]*"`);
  const out = text.replace(re, `$1"${version}"`);
  if (out === text) throw new Error(`No ${pkg} entry found in Cargo.lock`);
  return out;
}
```

- [ ] **Step 4: Run, verify pass**

Run: `npm test -- scripts/lib/version.test.ts -t "set"`
Expected: PASS for all three setter describe-blocks.

- [ ] **Step 5: Commit**

```bash
git add scripts/lib/version.mjs scripts/lib/version.test.ts
git commit -m "feat(release): per-file version setters"
```

---

## Task 3: `applyVersion` writer + CLI

**Files:**
- Modify: `scripts/lib/version.mjs`
- Test: `scripts/lib/version.test.ts`

- [ ] **Step 1: Grow the imports, then write the failing test for `applyVersion`**

First, update the imports at the top of `scripts/lib/version.test.ts`: add `beforeEach, afterEach` to the vitest import, add the node fs/os/path imports, and add `applyVersion` to the `./version.mjs` import:

```ts
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  nextVersion,
  setJsonVersion,
  setCargoTomlVersion,
  setCargoLockVersion,
  applyVersion,
} from "./version.mjs";
```

Then append to `scripts/lib/version.test.ts`:

```ts
describe("applyVersion", () => {
  let root: string;
  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), "gs-ver-"));
    mkdirSync(join(root, "src-tauri"), { recursive: true });
    writeFileSync(join(root, "package.json"), '{\n  "version": "0.0.0"\n}\n');
    writeFileSync(
      join(root, "src-tauri/tauri.conf.json"),
      '{\n  "version": "0.1.0"\n}\n',
    );
    writeFileSync(
      join(root, "src-tauri/Cargo.toml"),
      '[package]\nname = "glitch_spot"\nversion = "0.1.0"\n',
    );
    writeFileSync(
      join(root, "src-tauri/Cargo.lock"),
      'name = "glitch_spot"\nversion = "0.1.0"\ndependencies = []\n',
    );
  });
  afterEach(() => rmSync(root, { recursive: true, force: true }));

  it("writes the same version into all four files", () => {
    applyVersion("1.2.3", root);
    const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
    const conf = JSON.parse(
      readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"),
    );
    expect(pkg.version).toBe("1.2.3");
    expect(conf.version).toBe("1.2.3");
    expect(readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8")).toContain(
      'version = "1.2.3"',
    );
    expect(readFileSync(join(root, "src-tauri/Cargo.lock"), "utf8")).toContain(
      'name = "glitch_spot"\nversion = "1.2.3"',
    );
  });
});
```

- [ ] **Step 2: Run, verify failure**

Run: `npm test -- scripts/lib/version.test.ts -t applyVersion`
Expected: FAIL — `applyVersion` is not exported.

- [ ] **Step 3: Implement `applyVersion` + the CLI**

Add to `scripts/lib/version.mjs` (after the setters):

```js
function readCurrent(root) {
  const conf = JSON.parse(readFileSync(join(root, FILES.tauriConf), "utf8"));
  if (!conf.version) throw new Error("tauri.conf.json has no version");
  return conf.version;
}

export function applyVersion(version, root) {
  const edits = [
    [FILES.packageJson, setJsonVersion],
    [FILES.tauriConf, setJsonVersion],
    [FILES.cargoToml, setCargoTomlVersion],
    [FILES.cargoLock, setCargoLockVersion],
  ];
  for (const [rel, fn] of edits) {
    const p = join(root, rel);
    writeFileSync(p, fn(readFileSync(p, "utf8"), version));
  }
}

// CLI: `node scripts/lib/version.mjs current | next <bump> | apply <bump>`
if (import.meta.url === `file://${process.argv[1]}`) {
  const root = process.cwd();
  const [cmd, arg] = process.argv.slice(2);
  try {
    if (cmd === "current") {
      process.stdout.write(readCurrent(root));
    } else if (cmd === "next") {
      process.stdout.write(nextVersion(readCurrent(root), arg));
    } else if (cmd === "apply") {
      const v = nextVersion(readCurrent(root), arg);
      applyVersion(v, root);
      process.stdout.write(v);
    } else {
      process.stderr.write(
        "usage: version.mjs current | next <bump> | apply <bump>\n",
      );
      process.exit(2);
    }
  } catch (err) {
    process.stderr.write(`${err.message}\n`);
    process.exit(1);
  }
}
```

- [ ] **Step 4: Run, verify pass (full file green)**

Run: `npm test -- scripts/lib/version.test.ts`
Expected: PASS — every describe-block green.

- [ ] **Step 5: Verify the CLI against the real repo (no writes from `next`/`current`)**

Run: `node scripts/lib/version.mjs current && echo && node scripts/lib/version.mjs next patch`
Expected: prints `0.1.0` then `0.1.1`. Confirm `git status` shows no modified version files (only `next`/`current` were used — neither writes).

- [ ] **Step 6: Commit**

```bash
git add scripts/lib/version.mjs scripts/lib/version.test.ts
git commit -m "feat(release): applyVersion writer + version CLI"
```

---

## Task 4: The release orchestrator

**Files:**
- Create: `scripts/release.sh`
- Modify: `package.json`

- [ ] **Step 1: Write the release script**

Create `scripts/release.sh`:

```bash
#!/usr/bin/env bash
# ABOUTME: One-command local production release for Glitch Spot.
# ABOUTME: Bumps version, builds an unsigned universal DMG, publishes to GitHub Releases.
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
need node; need jq; need gh; need cargo; need git; need rustup

if [ "$DRY" -eq 0 ]; then
  branch="$(git rev-parse --abbrev-ref HEAD)"
  [ "$branch" = "main" ] || { echo "Releases must be cut from 'main' (currently on '$branch'). Use --dry-run to test elsewhere." >&2; exit 1; }
  [ -z "$(git status --porcelain)" ] || { echo "Working tree is not clean. Commit or stash first." >&2; exit 1; }
  gh auth status >/dev/null 2>&1 || { echo "gh is not authenticated. Run: gh auth login" >&2; exit 1; }
fi

if ! rustup target list --installed 2>/dev/null | grep -q '^x86_64-apple-darwin$'; then
  echo "Installing x86_64-apple-darwin Rust target (needed for the universal build)…"
  rustup target add x86_64-apple-darwin
fi

CURRENT="$(node scripts/lib/version.mjs current)"
NEW="$(node scripts/lib/version.mjs next "$BUMP")"
TAG="v$NEW"
echo "Release: $CURRENT -> $NEW (tag $TAG)$([ "$DRY" -eq 1 ] && echo ' [dry-run]')"

if [ "$DRY" -eq 0 ] && git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Tag $TAG already exists." >&2; exit 1
fi

# --- Version ---
node scripts/lib/version.mjs apply "$BUMP" >/dev/null

# --- Build (unsigned universal) ---
echo "Building universal DMG (compiles arm64 + x86_64; the first build is slow)…"
npm run tauri -- build --target universal-apple-darwin

DMG="$(ls -t src-tauri/target/universal-apple-darwin/release/bundle/dmg/*.dmg 2>/dev/null | head -1 || true)"
[ -n "$DMG" ] || { echo "No DMG was produced under target/universal-apple-darwin/release/bundle/dmg/." >&2; exit 1; }
ARTIFACT="/tmp/Glitch-Spot_${NEW}_universal.dmg"
cp "$DMG" "$ARTIFACT"
echo "Artifact: $ARTIFACT"

if [ "$DRY" -eq 1 ]; then
  echo "[dry-run] Reverting version changes."
  git checkout -- package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
  echo "[dry-run] Would commit, tag $TAG, push, create the release, and upload $ARTIFACT."
  exit 0
fi

# --- Commit + tag ---
git commit -aqm "chore(release): $TAG"
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
```

- [ ] **Step 2: Make it executable and add the npm alias**

Run: `chmod +x scripts/release.sh`

Modify `package.json` `scripts` — add the `release` entry alongside the others:

```json
    "release": "bash scripts/release.sh",
```

- [ ] **Step 3: Verify usage/guard errors without building**

Run: `npm run release` (no bump arg)
Expected: prints the usage line and exits non-zero (no build, no file changes).

Run: `bash scripts/release.sh patch` while on the `production-build-process` branch
Expected: fails fast with "Releases must be cut from 'main' … Use --dry-run". Confirms the branch guard. No build.

- [ ] **Step 4: Verify the full pipeline via `--dry-run` (real build, no publish)**

Run: `npm run release -- patch --dry-run`
Expected:
  - Prints `Release: 0.1.0 -> 0.1.1 (tag v0.1.1) [dry-run]`.
  - Installs `x86_64-apple-darwin` if missing, then builds (slow — both arches compile).
  - Prints `Artifact: /tmp/Glitch-Spot_0.1.1_universal.dmg` and the file exists (`ls -lh /tmp/Glitch-Spot_0.1.1_universal.dmg`).
  - Ends with the `[dry-run] Would commit…` line.
  - **`git status` is clean afterward** (version files were reverted; no tag created: `git tag | grep v0.1.1` returns nothing).

- [ ] **Step 5: Commit**

```bash
git add scripts/release.sh package.json
git commit -m "feat(release): one-command local release script"
```

---

## Task 5: README with install + release docs

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write the README**

Create `README.md`:

```markdown
# Glitch Spot

A macOS menu-bar debugging-overlay tool. Press a global hotkey, annotate the screen
with rectangles + notes, and hand the structured result to an AI coding agent or a
teammate.

## Install (from a GitHub Release)

Builds are **unsigned**, so macOS Gatekeeper will block the first launch.

1. Download `Glitch-Spot_<version>_universal.dmg` from the
   [latest release](https://github.com/dmakarenko/glitch-spot/releases/latest).
2. Open the DMG and drag **Glitch Spot** to **Applications**.
3. First launch: right-click **Glitch Spot.app** → **Open**, then confirm. (A normal
   double-click only offers "Move to Trash".) Or clear the quarantine flag manually:

   ```
   xattr -dr com.apple.quarantine "/Applications/Glitch Spot.app"
   ```

The app runs in the menu bar — there is no Dock icon or app window.

## Development

```bash
PATH="$HOME/.cargo/bin:$PATH" npm run tauri -- dev
```

See `CLAUDE.md` for details.

## Cutting a release

Releases are built and published locally (no CI). From a clean `main`:

```bash
npm run release -- patch     # or minor | major | an explicit X.Y.Z
```

This bumps the version across `package.json`, `src-tauri/tauri.conf.json`,
`src-tauri/Cargo.toml`, and `src-tauri/Cargo.lock`; commits and tags `vX.Y.Z`; builds an
unsigned **universal** (arm64 + x86_64) DMG; pushes; and creates the GitHub Release with
the DMG attached and auto-generated notes.

Dry-run the whole thing (builds the DMG, makes no git/GitHub changes — works off `main`):

```bash
npm run release -- patch --dry-run
```
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: README with install + release instructions"
```

---

## Self-Review notes (for the worker)

- **Spec coverage:** unsigned build (Task 4 build step + README/release-body note), GitHub Releases publish (Task 4), universal arch (Task 4 `--target universal-apple-darwin` + x86_64 target install), one-command + bump (Tasks 3–4), version-file sync across all four files (Tasks 2–3), clean-`main` guard (Task 4), auto notes (Task 4 `--generate-notes`), `--dry-run` (Task 4), brew-deferred seam (publish stage isolated in Task 4 — no extra code), testing of version logic (Tasks 1–3). All seed requirements map to a task.
- **No auto-update / no brew / no signing** — intentionally absent per the seed's non-goals.
- The DMG path is **globbed** (`ls -t … *.dmg | head -1`), not hardcoded, so a change in Tauri's default artifact name won't break publishing.
```
