---
date: 2026-10-28
pr: dmakarenko/glitch-spot#5
branch: sec-snapshot-privacy-20261028
---

# Owner-only snapshot files and an honest redaction prompt

## Engineering
- `src-tauri/src/snapshot/persist.rs`: `write()` creates each `{id}/` folder with mode 0700 and each file with 0600. The first successful `snapshots_dir()` call per process sets 0700 on the root and each snapshot folder, and 0600 on each file in them, so snapshots saved before the upgrade are covered. The sweep does not follow symlinks, and a failed chmod on one entry is logged and skipped.
- `src-tauri/src/snapshot/mod.rs`: the agent prompt names the three redaction covers. Solid and branded covers do not depend on the source pixels. Pixelated, the default, keeps the average color of each block.
- No `lib.rs` change and no change to the snapshot JSON schema.

## User value
Other local users and processes can no longer read saved screenshots and notes in `~/.glitch-spot/snapshots/`. A coding agent now knows that a pixelated redaction can still show coarse color and layout.

## Initiative positioning
This is one of the fix stages of the Glitch Spot security and performance review. It closes the snapshot privacy finding. The other fix-now findings ship in their own stages.
