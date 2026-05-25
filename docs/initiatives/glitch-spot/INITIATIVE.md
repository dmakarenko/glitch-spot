# Initiative: Glitch Spot

> Desktop debugging-overlay tool. Press a global hotkey, annotate the screen with rectangles + notes, hand the structured result to an AI coding agent or a teammate.

**Goal:** A snappy, AI-agent-friendly visual feedback tool for dev-time use. Hotkey → annotate → JSON-on-clipboard (or short-lived link). Eventually multi-platform; macOS-first.

**Current focus:** between stages — v0.3 shipped; v1.0-tunnel-share is next on the roadmap, no plan drafted yet.

**Cross-stage scope reference:** [`scope.md`](./scope.md). Architectural decisions, snapshot schema, version slicing.

**Original PRD (historical):** [`docs/seeds/2026-05-21-glitch-spot-seed.md`](../../seeds/2026-05-21-glitch-spot-seed.md).

## Stages

- [x] **v0.1-foundation** — Tauri 2 macOS scaffold, hotkey-triggered overlay, transparent live mode (`Ctrl+Opt+Cmd+G`) + frozen screenshot mode (`Ctrl+Opt+Cmd+H`), work-area positioning, click-to-dismiss. Branch: `v0.1-foundation`. Plan: [`docs/plans/completed/2026-05-21-glitch-spot-v0.1-foundation.md`](../../plans/completed/2026-05-21-glitch-spot-v0.1-foundation.md). Merged via `3301866`.
- [x] **v0.2-drawing** — drag-to-draw rectangles with floating inline notes, undo, delete-on-hover, Save (`Cmd+Return`) / Discard (`Esc`-with-content modal), snapshot JSON to clipboard, annotated PNG generation, min-size guard against accidental clicks, text-selection suppressed during drags. Both modes supported. Branch: `v0.2-drawing`. Main plan: [`docs/plans/completed/2026-05-22-glitch-spot-v0.2-drawing.md`](../../plans/completed/2026-05-22-glitch-spot-v0.2-drawing.md). Annotated-PNG follow-up: [`docs/plans/completed/2026-05-22-glitch-spot-v0.2-annotated-png.md`](../../plans/completed/2026-05-22-glitch-spot-v0.2-annotated-png.md). Merged via `68c328d`.
- [x] **v0.4-mcp** — local MCP HTTP server on `127.0.0.1:7820` with `wait_for_snapshot` / `get_latest_snapshot` / `get_snapshot(id)` / `cancel_wait` tools, `/glitch-spot` CC skill, menu-bar tray app (no Dock icon, autostart, dynamic snapshot-age label), standalone eval binary + bash eval script. Seed: [`v0.4-coding-harness-seed.md`](./v0.4-coding-harness-seed.md). Plans: [`docs/plans/completed/2026-05-22-glitch-spot-v0.4-mcp.md`](../../plans/completed/2026-05-22-glitch-spot-v0.4-mcp.md), [`docs/plans/completed/2026-05-22-menu-bar-app.md`](../../plans/completed/2026-05-22-menu-bar-app.md), [`docs/plans/completed/2026-05-22-glitch-spot-skill-evals.md`](../../plans/completed/2026-05-22-glitch-spot-skill-evals.md). Merged via `c23c8d3`.
- [x] **v0.3-persistence** — snapshots persist to `~/.glitch-spot/snapshots/{id}/{clean.png, annotated.png, snapshot.json}`; mtime-based 24h sweep on app startup; `build` refactored to a pure function returning `BuiltSnapshot` with all I/O moved to a new `snapshot::persist` module; end-to-end integration test added. Seed: [`v0.3-persistence-seed.md`](./v0.3-persistence-seed.md). Plan: [`docs/plans/completed/2026-05-25-v0.3-persistence.md`](../../plans/completed/2026-05-25-v0.3-persistence.md). Merged via `edb8c6b`.
- [ ] **v1.0-tunnel-share** — share annotated snapshots with teammates via a tunneled local HTTP server. Extend the existing axum server (port 7820, today serving MCP) with read-only `GET /share/{id}/...` routes for `snapshot.json` + `clean.png` + `annotated.png`, then surface a shareable URL (clipboard + tray menu) backed by one of: ngrok (one-click, requires account), Tailscale Funnel (zero third-party for teams on Tailscale), or cloudflared (zero-cost ephemeral URLs). No cloud relay, no R2, no cross-machine persistence — the snapshot stays on the developer's machine for the lifetime of the share. Tunnel choice, auth model (passcode? expiring path token?), and share lifetime are open questions for the brainstorm.
- [ ] **later: cloud relay + web SDK** — Cloudflare Workers + R2 with TTL'd passcode-protected links and HTML view, plus `@glitch-spot/web` SDK for DOM-path enrichment in web hosts. _Postponed 2026-05-25 — superseded by `v1.0-tunnel-share` for the cross-machine-sharing use case. Revisit if the tunnel approach proves insufficient (durable public links, web-host DOM enrichment as a primary need)._
- [ ] **future-ports** — Windows desktop app, Linux desktop app (PipeWire capture). _Deferred indefinitely 2026-05-25 — staying macOS-first while we focus on feature depth over OS breadth._
- [ ] **v1.5+** — source-map → file:line resolution, mobile SDKs, public-link redaction, etc.

## Open questions / known deferrals

- **Overlay above menu bar (macOS):** _Won't fix._ Work-area positioning is the accepted design — clicks on the menu bar / dock pass through to the system surfaces and that is intentional. Decided 2026-05-22.
- **Esc dismissal:** borderless transparent NSWindows don't always become the key window; Esc may not fire. Click-anywhere-to-dismiss is the reliable v0.1 path.
- **Hotkey conflict handling:** if a default hotkey is taken by another app, registration panics at startup. Needs graceful UX in a later slice (v0.4 or v1.0).
- **"No monitor at cursor" inconsistency:** capture falls back to display 0; overlay returns `NoMonitor` error. Picked: reconcile when revisiting work-area code.
- **Tauri Monitor ↔ SCDisplay index alignment:** assumed (both back onto `CGGetActiveDisplayList`) but not formally verified. Hardening pass deferred.
- **Source-map → file:line resolution:** the real differentiator. v1.5+ slice.

## Working agreements (within this initiative)

- Each stage gets its own git branch (`v0.X-name`), its own plan in `docs/plans/`, and merges to `main` independently.
- Stage states: `[ ]` not-started → `[~]` in-progress / branch open → `[x]` PR merged to `main` (append PR ref when ticking).
- When all stages ship, `mv docs/initiatives/glitch-spot docs/initiatives/completed/glitch-spot`.
