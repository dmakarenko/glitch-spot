# Initiative: Glitch Spot

> Desktop debugging-overlay tool. Press a global hotkey, annotate the screen with rectangles + notes, hand the structured result to an AI coding agent or a teammate.

**Goal:** A snappy, AI-agent-friendly visual feedback tool for dev-time use. Hotkey → annotate → JSON-on-clipboard (or short-lived link). Eventually multi-platform; macOS-first.

**Current focus:** `v0.2-drawing` — rectangle drawing with inline notes + clipboard JSON save.

**Cross-stage scope reference:** [`scope.md`](./scope.md). Architectural decisions, snapshot schema, version slicing.

**Original PRD (historical):** [`docs/seeds/2026-05-21-glitch-spot-seed.md`](../../seeds/2026-05-21-glitch-spot-seed.md).

## Stages

- [~] **v0.1-foundation** — Tauri 2 macOS scaffold, hotkey-triggered overlay, transparent live mode (`Ctrl+Opt+Cmd+G`) + frozen screenshot mode (`Ctrl+Opt+Cmd+H`), work-area positioning, click-to-dismiss. Branch: `v0.1-foundation`. Plan: [`docs/plans/2026-05-21-glitch-spot-v0.1-foundation.md`](../../plans/2026-05-21-glitch-spot-v0.1-foundation.md). Awaiting merge to `main`.
- [~] **v0.2-drawing** — drag-to-draw rectangles with floating inline notes, undo, delete-on-hover, Save (`Cmd+Return`) / Discard (`Esc`-with-content modal), snapshot JSON to clipboard. Both modes supported. Branch: `v0.2-drawing`. Plan: [`docs/plans/2026-05-22-glitch-spot-v0.2-drawing.md`](../../plans/2026-05-22-glitch-spot-v0.2-drawing.md).
- [ ] **v0.3-persistence** — write snapshot JSON + `clean.png` + `annotated.png` to `~/.glitch-spot/snapshots/{id}/`, 24h auto-cleanup.
- [ ] **v0.4-mcp** — local MCP server (stdio child binary or HTTP, TBD), `wait_for_snapshot` / `get_latest_snapshot` / `get_snapshot(id)` tools + resources, Cursor + Claude Code config snippets.
- [ ] **v1.0-windows-and-relay** — Windows port of the desktop app, `@glitch-spot/web` SDK for DOM-path enrichment in web hosts, Cloudflare relay (Workers + R2) with TTL'd passcode-protected links and HTML view.
- [ ] **v1.5+** — Linux, source-map → file:line resolution, mobile SDKs, public-link redaction, etc.

## Open questions / known deferrals

- **Overlay above menu bar (macOS):** v0.1 uses work-area positioning, excluding menu bar and dock. Clicks pass through to those strips. Fixing requires NSWindow level above `NSStatusWindowLevel` via private API.
- **Esc dismissal:** borderless transparent NSWindows don't always become the key window; Esc may not fire. Click-anywhere-to-dismiss is the reliable v0.1 path.
- **Hotkey conflict handling:** if a default hotkey is taken by another app, registration panics at startup. Needs graceful UX in a later slice (v0.4 or v1.0).
- **"No monitor at cursor" inconsistency:** capture falls back to display 0; overlay returns `NoMonitor` error. Picked: reconcile when revisiting work-area code.
- **Tauri Monitor ↔ SCDisplay index alignment:** assumed (both back onto `CGGetActiveDisplayList`) but not formally verified. Hardening pass deferred.
- **Source-map → file:line resolution:** the real differentiator. v1.5+ slice.

## Working agreements (within this initiative)

- Each stage gets its own git branch (`v0.X-name`), its own plan in `docs/plans/`, and merges to `main` independently.
- Stage states: `[ ]` not-started → `[~]` in-progress / branch open → `[x]` PR merged to `main` (append PR ref when ticking).
- When all stages ship, `mv docs/initiatives/glitch-spot docs/initiatives/completed/glitch-spot`.
