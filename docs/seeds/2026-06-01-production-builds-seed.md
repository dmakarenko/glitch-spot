# Production builds → GitHub Releases (design)

> Status: design approved, pre-plan. Standalone seed (not tracked as an initiative stage).
> Date: 2026-06-01. Branch: `production-build-process`.

## Goal

Produce distributable macOS builds of Glitch Spot **locally** (no CI) and publish the
final artifact to **GitHub Releases**, driven by a single command. Lay the groundwork
so a Homebrew tap can be added later without reworking the pipeline.

## Decisions (locked during brainstorming)

| Axis | Decision | Consequence |
|------|----------|-------------|
| Code signing | **Unsigned / ad-hoc** — no Apple Developer account | Users clear Gatekeeper on first launch (right-click → Open, or `xattr -dr com.apple.quarantine`). No recurring cost. |
| Distribution channel | **GitHub Releases** on the private `dmakarenko/glitch-spot` repo | `gh` CLI uploads the artifact; downloads require repo access (private). |
| Homebrew | **Deferred** | Blocked by private repo (brew can't fetch private release assets). Pipeline keeps a clean seam to add it later. |
| Architecture | **arm64 only** (Apple Silicon) | `tauri build` (host arch). Revised from universal — see Revision note. |
| Release trigger | **One command** `./release.sh [patch\|minor\|major\|X.Y.Z]` | Bumps version, commits, tags, builds, publishes in one shot. |
| Branch guard | **Require clean `main`** | Releases only cut from the integration branch. Relaxed under `--dry-run`. |
| Release notes | **`gh release create --generate-notes`** | Auto-generated from commits/PRs since last tag. |
| Auto-update | **None** (no Tauri updater) | Out of scope. Users update by downloading the new DMG. |

## Non-goals

- No CI / GitHub Actions.
- No code signing or notarization.
- No Homebrew tap (designed-for, not built).
- No in-app auto-update.
- No Windows/Linux artifacts (macOS arm64 only).

## Revision (2026-06-02): universal → arm64-only

The initial decision was a universal (arm64 + x86_64) build. During implementation the
universal build failed at the bundling stage: the crate ships a second binary,
`mcp_server` (an eval/dev-only tool; the app runs MCP in-process), and **Tauri's bundler
enumerates every `[[bin]]` from cargo metadata and copies each into the `.app`** — but it
only lipo-merges the *primary* binary for universal builds, so the missing universal
`mcp_server` aborts bundling. A `required-features` gate did **not** help (Tauri still
expects the binary from metadata and fails when it's absent). Verified empirically with a
single-arch build.

Decision (Dmitry): build **arm64-only**. Single-arch has no lipo step, so the compiled
`mcp_server` copies fine and bundling succeeds. The shipped `.app` includes the small
(unused) `mcp_server` binary — acceptable for a personal tool. Intel Macs are not
supported. (If universal is ever needed, the clean fix is to move `mcp_server` into its
own workspace crate so Tauri never sees it.)

## Architecture

A single bash orchestrator, `scripts/release.sh`, plus an `npm run release` alias.
Version bumping uses the **native tool per file** rather than hand-rolled text munging:

- `package.json` → `npm version <ver> --no-git-tag-version`
- `tauri.conf.json` → `jq` (source of truth for the *current* version)
- `src-tauri/Cargo.toml` (+ `Cargo.lock`) → `cargo set-version <ver>` (cargo-edit)

### Stages (fail-fast; each guarded)

1. **Pre-flight**
   - On `main` (unless `--dry-run`), working tree clean.
   - `gh auth status` succeeds.
   - Required tools present: `node`, `jq`, `gh`, `cargo`, `git`.
2. **Version**
   - Read current version from `tauri.conf.json`.
   - Compute next version from the argument (`patch`/`minor`/`major` bump, or explicit `X.Y.Z`).
   - Write the **same** version to all three files (correcting `package.json`'s `0.0.0` drift).
3. **Commit + tag**
   - `git commit -am "chore(release): vX.Y.Z"`, `git tag vX.Y.Z`.
   - Fail if the tag already exists.
4. **Build**
   - `tauri build` (host arm64, unsigned/ad-hoc).
5. **Publish**
   - `git push` branch + tag.
   - `gh release create vX.Y.Z --generate-notes --notes-file <body>` (body prepends the unsigned-app install note).
   - Upload the arm64 `.dmg`.

### `--dry-run`

Runs pre-flight (minus the clean-`main` guard) + version compute + build, prints the
planned tag/version/artifact, and makes **no** git or GitHub changes. This is the local
substitute for CI: verify the full build before cutting a real release. Version files are
written to a scratch copy (or reverted) so a dry run leaves the tree clean.

## Artifact

- Source: `src-tauri/target/release/bundle/dmg/`.
- Tauri's default name has spaces (`Glitch Spot_X.Y.Z_aarch64.dmg`); rename to a
  URL-clean **`Glitch-Spot_X.Y.Z_aarch64.dmg`** for the upload.
- Only the `.dmg` is uploaded (it contains the `.app`).

### Install note (release body + README)

```
This build is unsigned. On first launch macOS will block it.
Right-click the app → Open (then confirm), or run:
  xattr -dr com.apple.quarantine "/Applications/Glitch Spot.app"
```

## Brew-ready seam (future, not built)

Stage 5 isolates "publish artifact" from build/version logic. Adding brew later means a
new `--tap` step that pushes the DMG + an updated cask `.rb` to a public
`homebrew-glitch-spot` repo — no change to stages 1–4. Documented here so the seam is intentional.

## Testing

Per TDD:

- **Version compute + sync** is extracted into a pure, testable unit. A test runs it
  against temp copies of the three version files and asserts: (a) all three land on the
  same target version, (b) `patch`/`minor`/`major` math is correct, (c) the `0.0.0`
  `package.json` drift is corrected. (Vitest is the project's test runner; if shell-native
  testing is cleaner for a bash function, a `bats`-style or shell assertion harness is acceptable —
  decided at plan time.)
- **Full pipeline** verified via `--dry-run` against the real tree (no release created).
- Shell stages that only orchestrate external tools (`gh`, `tauri`) are not unit-tested;
  they're exercised by the dry run and a real first release.

## Open items for the plan

- Exact location/extraction of the version-sync function so it's unit-testable
  (standalone `scripts/lib/version.sh` sourced by `release.sh`, vs a tiny node helper).
- Whether to also `git push` requires confirming `main` tracks `origin/main`.
- First real release version: bump `0.1.0 → 0.1.1`? Or mint `0.2.0`? (Confirm at release time, not in code.)
