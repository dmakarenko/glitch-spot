# Glitch Spot — Working Product Scope

**Date:** 2026-05-21
**Status:** Draft — working scope, supersedes the v1-scope counter-proposal framing
**Related:** original PRD at `2026-05-21-glitch-spot-seed.md`

---

## 1. What we are building

A tool the team uses during development to capture visual feedback against a running app and hand it to an AI coding agent — either directly through MCP on the same machine, or via a short-lived link the agent (or a teammate) can pull from.

Two ways to think about it:

- **For the human:** a global hotkey that pauses your screen, lets you draw rectangles and write notes on top of any running app, and produces a structured artifact when you're done.
- **For the agent:** an MCP tool (`glitch_spot.wait_for_snapshot`) that blocks until the human finishes a capture session, then returns a rich object describing what they marked up and what context the app exposed.

---

## 2. The main loop

1. The app being debugged is running. It can be a web app in a browser, a desktop app, a mobile simulator — anything visible on screen.
2. An AI agent (Cursor / Claude Code / etc.) optionally launches the dev server itself, then calls `glitch_spot.wait_for_snapshot()`. The call blocks.
3. The developer hits a global hotkey. Glitch Spot's overlay appears on top of the active window (or full screen).
4. They draw one or more rectangles. Each rectangle can have a text note. They can edit / undo before finishing.
5. They close the overlay (Done / Esc). Glitch Spot:
    - Grabs a clean screenshot (without our overlay) and an annotated screenshot (with rectangles burned in).
    - Assembles a JSON snapshot object (schema in §5).
    - **Always:** copies the full snapshot JSON to the clipboard.
    - **If relay configured:** uploads the snapshot to the relay, gets back a short URL with optional passcode, also copies that URL to the clipboard.
    - **If an MCP client is waiting:** unblocks `wait_for_snapshot` and returns the snapshot.
6. The agent now has either the JSON directly (via MCP) or a link to fetch it. The developer's job is done.

---

## 3. Components

Three deployables in total. v0 ships only the first.

| Component | What it is | Where it runs | v0 | v1 |
|---|---|---|---|---|
| **Desktop app** | The product. Screen capture, overlay + drawing, snapshot assembly, embedded MCP server, clipboard writer, relay client. | Each developer's machine. | ✅ macOS | ✅ macOS + Windows |
| **Web SDK** (`@glitch-spot/web`) | npm package the host web app imports. Exposes `setContext()` for runtime-state injection, and a small local-WS endpoint the desktop app calls during snapshot assembly to read DOM path + component name. | Inside the host web app, in the browser. | — | ✅ |
| **Relay** | Cloudflare Worker + R2. Hosts snapshots and rendered images for a TTL; serves a JSON-or-HTML view at `/s/{id}`. | Cloudflare edge. | — | ✅ |

The MCP server is **not** a separate component — it lives inside the desktop app and is the desktop app's primary outbound interface to coding agents.

```
                         ┌──────────────────────┐
                         │   Coding agent       │
                         │   (Cursor / Claude   │
                         │    Code / etc.)      │
                         └────┬───────────┬─────┘
                              │ MCP       │ HTTPS
                              │ (local)   │ (cross-machine)
                              ▼           ▼
        ┌─────────────────────────┐   ┌──────────────────┐
        │  Desktop app            │──▶│  Relay           │
        │                         │   │  (Cloudflare)    │
        │  • Screen capture       │   │                  │
        │  • Overlay + drawing    │   │  • Snapshot KV   │
        │  • Snapshot builder     │   │  • Image bucket  │
        │  • MCP server           │   │  • HTML view     │
        │  • Relay client         │   └──────────────────┘
        │  • Clipboard writer     │
        └────────────┬────────────┘
                     │ Local WS
                     │ (only when web SDK is present)
                     ▼
        ┌─────────────────────────┐
        │  Host web app           │
        │                         │
        │  @glitch-spot/web       │
        │   • setContext()        │
        │   • DOM resolver        │
        └─────────────────────────┘
```

Communication paths:

- **Agent ↔ desktop app:** MCP (stdio or localhost HTTP) — for snapshot delivery and blocking `wait_for_snapshot`.
- **Desktop app ↔ host web app:** local WebSocket on a fixed port, only invoked during snapshot assembly when the web SDK is detected.
- **Desktop app → relay:** HTTPS POST (per-user API key) to upload snapshot JSON + clean/annotated images.
- **Agent or teammate → relay:** HTTPS GET on `/s/{id}` (optionally with passcode) — returns JSON by default, HTML with `?view=html`.

---

## 4. Versions

### v0.1 — prototype (the only thing we build first)

Single-user, single-machine. macOS only. No backend. No web SDK. No relay.

Stack: **Tauri 2** (chosen — see §11). Cross-platform from day one in code, but Windows packaging/testing is deferred until v1.

- Desktop application (menu-bar resident) with a global hotkey.
- Captures whole screen or active window using OS screen-capture APIs.
- Overlay window for drawing rectangles + attaching text notes per rectangle.
- Multi-step undo within a session; single-level redo is fine.
- On finish: serialize the snapshot to JSON, copy to clipboard. Done.
- Local MCP server (stdio or local HTTP) exposing `wait_for_snapshot`, `get_latest_snapshot`, `get_snapshot(id)`.
- No SDK in any host app yet. Context is whatever we can read from the OS (window title, app name, screen size) — nothing app-internal.

This is enough to be useful to one person + one agent on one machine. Everything else is built on top.

### v1 — usable with the team

Adds cross-machine handoff, richer context for web hosts, and the second OS.

- **Windows** support for the desktop app (macOS shipped in v0.1).
- **Cloudflare-hosted relay.** Snapshots optionally upload to a Worker + R2 bucket (or equivalent). Short URL returned, copied to clipboard.
  - URL format: `https://glitch-spot.app/s/{id}`. Default response: snapshot JSON. With `?view=html`: human-readable HTML page rendering the annotated screenshot, notes, and context.
  - Optional 6-digit passcode per snapshot. Default TTL 15 min, max 24h.
  - The two image URLs in the snapshot point at the same relay (`/i/{id}/clean.png`, `/i/{id}/annotated.png`).
- **Web SDK** (`@glitch-spot/web`): drop-in npm package. When present in a web app, enriches the snapshot with DOM-path and framework-component info for each rectangle, plus app-injected custom context (see §8).

### v1.5+ — explicitly deferred

Listed so we don't accidentally build them in v1:

- **Linux** support for the desktop app.
- Source-map → file:line resolution.
- Git-blame enrichment (last commit per file:line).
- Mobile SDKs (iOS / Android / React Native / Flutter).
- Multi-turn AI debugging (agent replying back into the overlay).
- Automatic PII redaction.
- Public anonymous shareable links.

---

## 5. Out of scope (cut decisions)

These are not "we'll get to them" — they are deliberate omissions for the working scope:

- Customer-facing bug reporting (production tool for non-developers). Glitch Spot is a dev-time tool.
- Free-form circles / arrows / brushes. Rectangles only. One primitive.
- Live AI annotations replying back onto the canvas. The agent reads, edits files, opens a PR — it does not write back to the overlay.
- LLM "prompt synthesis" that paraphrases or numerically interprets the user's note. We pass the user's note verbatim. The IDE agent interprets it.

---

## 6. The snapshot object

This is the contract. Everything else serves this.

```json
{
  "id": "snap_01HXYZABCD",
  "createdAt": "2026-05-21T14:32:11Z",
  "ttlSeconds": 900,
  "annotations": [
    {
      "id": "anno_1",
      "shape": "rect",
      "coords": { "x": 140, "y": 310, "w": 280, "h": 60 },
      "note": "make this smaller and match the login button color",
      "domPath": null,
      "component": null
    }
  ],
  "context": {
    "capture": {
      "mode": "screen",                         // "screen" | "window" | "web"
      "screen": { "w": 1440, "h": 900, "scale": 2 },
      "window": { "title": "Checkout — localhost:3000", "appName": "Google Chrome" }
    },
    "sdk": null,                                 // populated when a Glitch Spot SDK is running
    "os": { "platform": "macos", "version": "15.4.0" }
  },
  "images": {
    "clean":     "https://glitch-spot.app/i/snap_01HXYZABCD/clean.png",
    "annotated": "https://glitch-spot.app/i/snap_01HXYZABCD/annotated.png"
  }
}
```

When the web SDK is present, `context.sdk` is populated:

```json
"sdk": {
  "appName": "checkout-frontend",
  "route": "/checkout",
  "viewport": { "w": 1280, "h": 800 },
  "framework": "react",
  "git": { "branch": "feat/checkout-redesign", "head": "a1b2c3d", "dirty": true },
  "console": [ /* last 50 lines, trimmed */ ],
  "network": [ /* last 20 requests, headers only */ ],
  "custom": { /* whatever the app passed to glitchSpot.setContext(...) */ }
}
```

And each annotation gains DOM/component info:

```json
"domPath": "main > section.checkout > form > div.price-row > label.PriceLabel",
"component": { "name": "PriceLabel", "framework": "react" }
```

In v0, `context.sdk` is always `null` and `images.{clean,annotated}` are absent (clipboard-only). v1 fills both in.

---

## 7. Capture modes

- **Screen / window** — works for any visible application. The default in v0.1. Glitch Spot uses OS screen-capture APIs via Rust bindings (macOS: ScreenCaptureKit via `screencapturekit` crate; Windows: Windows.Graphics.Capture via `windows-capture` crate; Linux: PipeWire — v1.5+).
  - The overlay window is excluded from capture so the clean screenshot truly is clean.
  - The annotated screenshot is rendered by compositing the clean shot with the annotation layer.
- **Web** — only when `@glitch-spot/web` is installed in the host app and active in the current page. Lets us read DOM under each bounding box without round-tripping through the OS. Used as an enrichment, not a replacement — screen capture still produces the images.

---

## 8. SDK surface (web, v1)

Minimal API. The SDK does not draw the overlay — the desktop app does. The SDK only exposes context and identifies DOM nodes when asked.

```ts
import { glitchSpot } from "@glitch-spot/web";

glitchSpot.init({
  appName: "checkout-frontend",
  framework: "react"  // optional, auto-detected when possible
});

// In any component / route handler, push state that should ride along on the next snapshot:
glitchSpot.setContext({
  current_view_controller: "ProductDetailsPage",
  user_authentication_state: "logged_in",
  active_experiments: ["new_checkout_flow_v2"]
});
```

Capture happens through the desktop app, not the SDK. The SDK's job is to expose a small local websocket the desktop app talks to when a snapshot is being assembled — that's how DOM-path resolution happens.

---

## 9. MCP server contract

Local, runs inside the desktop app. Speaks the standard MCP protocol over stdio (for IDE-local agents) and a localhost HTTP transport (for browser-based agents).

Tools:

- `glitch_spot.wait_for_snapshot(timeout_seconds?: number)` — blocks until the next snapshot is finalized, returns the full snapshot JSON.
- `glitch_spot.get_latest_snapshot()` — returns the most recently finalized snapshot (no block).
- `glitch_spot.get_snapshot(id)` — returns a specific snapshot by ID.

Resources:

- `glitch-spot://snapshots/latest` — same content as `get_latest_snapshot`, but as an MCP resource.
- `glitch-spot://snapshots/{id}` — same as `get_snapshot`.

---

## 10. Relay (v1)

- Cloudflare Worker + R2 (or D1 + R2). Snapshot JSON in KV/D1 keyed by `id`. Images in R2.
- Upload from the desktop app over HTTPS using a per-team API key.
- TTL enforced at read time and via R2 lifecycle rules.
- `GET /s/{id}` → JSON by default. `GET /s/{id}?view=html` → HTML view.
- Passcode (when set) required as `?p=######` or `X-Glitch-Spot-Passcode` header.
- The relay knows nothing about teams or auth in v0/v1 beyond "this API key is allowed to write." Team-level access control is a later concern.

---

## 11. Tech-stack decisions

- **Desktop app stack — Tauri 2 (chosen).** Rust core, system webview for the UI. Lean binary (~3-10 MB), modern, and the right long-term posture even though screen capture isn't in core.
  - Screen capture on macOS: `screencapturekit` Rust binding (or thin FFI over ScreenCaptureKit). Windows (v1): `windows-capture` crate or direct Windows.Graphics.Capture FFI. Linux (v1.5+): PipeWire.
  - Overlay window: one borderless, transparent, always-on-top Tauri window per monitor, sized to the display. Drawing in the webview via HTML Canvas — Rust-side just owns the window lifecycle and screenshot capture.
  - Frontend framework inside the webview: TBD (Svelte / Solid / vanilla — pick the lightest thing that gives us decent canvas ergonomics).
  - Global hotkey: `tauri-plugin-global-shortcut`.
  - Clipboard: `tauri-plugin-clipboard-manager`.
- **MCP transport (v0.1):** long-running localhost HTTP server started by the desktop app. Fits the always-running desktop-app model and lets multiple agents subscribe simultaneously. Stdio support can come later if a specific IDE needs it.
- **Local-WS bridge to web SDK (v1):** Rust-side `axum`/`tokio-tungstenite` WebSocket server on a fixed local port (e.g. 7821) listening for connections from `@glitch-spot/web` instances. Used only during snapshot assembly when the web SDK is detected.

Things to figure out as we go, not blockers:

- Whether the overlay webview's transparency works cleanly across all macOS versions we care about (some past Tauri versions had quirks with `transparent: true` on macOS 14+).
- How we handle multi-monitor: one window per display vs. one window spanning the virtual desktop. Per-display is likely simpler and gives crisper DPI handling.

I have opinions on all of these but don't want to bake them in without your input — they're the next decisions on the table.

---

## 12. Open product questions

- Should the overlay let you draw on top of the *live* app (where the app keeps responding to events behind the overlay), or does it freeze the visible content at activation time? Freeze is much simpler; live is nicer if you want to keep clicking around while annotating.
- When you draw multiple rectangles, should each one get its own note input, or one shared note for the whole session? My current default is per-rectangle.
- Should the annotated image render note text next to each rectangle, or just the rectangles (text only in JSON)? Embedding text in the image makes the HTML view nicer but the rectangles uglier.
- For the `wait_for_snapshot` MCP tool: what's a sensible default timeout? Suggesting 10 minutes — long enough to switch context, short enough not to leak forever.
