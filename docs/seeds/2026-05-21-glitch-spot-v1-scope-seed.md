# Glitch Spot — Tighter v1.0 Scope (Counter-Proposal Seed)

**Date:** 2026-05-21
**Status:** Draft — counter-proposal to `2026-05-21-glitch-spot-seed.md`
**Author:** Dmitry + Claude (critique session)

---

## 0. Why this file exists

The original PRD (`2026-05-21-glitch-spot-seed.md`) promises three platforms, an AI synthesis engine, an MCP server, public shareable links with passcodes, automatic PII redaction, and live multi-turn AI debugging. It also picks a competitor set (Jam, Marker, Webvizio) that flatters the gap analysis.

This seed sketches what a v1.0 would look like if we **picked the one defensible wedge** the original PRD buried in "Phase 1.5", went deep on it, and explicitly scoped out the parts that are either commoditizing fast (MCP) or strategically weak (cross-platform breadth, public shareable links, hand-wavy PII redaction).

---

## 1. The single thesis

**Bounding-box-to-source-file is the only defensible primitive in this space right now.**

Everything else in the PRD (annotation overlay, console capture, MCP server, prompt synthesis) is either already shipped by incumbents (Jam, Sentry) or trivially copy-able within a quarter. The hard, valuable thing — the thing that turns a screenshot into a *change* an AI agent can make — is the deterministic mapping from "this rectangle on screen" to "this React component in `apps/web/src/features/checkout/PriceLabel.tsx:42`."

If we ship that well, we have a moat. If we don't, we're "Jam + MCP," which is a feature, not a company.

---

## 2. What we are NOT building in v1.0

Cut ruthlessly. Each of these is a real cost, not a feature gap.

| Cut | Why |
|---|---|
| **Mobile SDK (iOS/Android/RN/Flutter)** | Distribution is brutal (app review, security audits, app size). Defer to v1.5 once web wedge is proven. |
| **Desktop overlay (Tauri/Electron, global click-through)** | Tiny target market, large platform engineering cost (accessibility entitlements per OS), unclear ICP. Kill or push to v2.0+. |
| **Public shareable links with 4-digit passcodes** | Security model is a B2B blocker, not a footnote. Replace with workspace-scoped auth (see §5). |
| **Automatic PII redaction at the text-node level** | This is a research problem, not a feature. Promising it in v1 invites security teams to tear the product apart. Ship without it; tell customers to use it on dev/staging only. |
| **"LLM Prompt Synthesis Engine" that paraphrases human intent** | The PRD's own example ("reduce dimensions by approximately 15%") hallucinates specificity the user didn't give. v1 ships the user's *literal* annotation text, plus structured context. Let the IDE agent do the synthesis. |
| **Multi-step undo** | Single-level undo is enough for a 30-second annotation session. |
| **Free-form circles** | Rectangles only. One shape primitive. |
| **Resizable/draggable annotations** | Draw-once. If you mis-drew, undo and redraw. |
| **"60 FPS, <2% CPU overhead" NFRs** | Aspirational without engineering grounding. v1 ships with measured numbers, not promised ones. |

---

## 3. What v1.0 IS

### 3.1 One platform: Web SDK + browser-side capture

- npm package, drop-in script tag, zero-config defaults.
- Activated by keyboard shortcut (`Cmd+Shift+G` or similar) — no floating button cluttering the host app.
- Draws one HTML5 canvas overlay. Rectangles only. One annotation per session can have a text note. That's it.

### 3.2 The core primitive: bounding box → source file + line

This is the work. Everything else is table stakes around it.

- During capture, walk the DOM under the bounding box and identify the **deepest React/Vue/Svelte component** that owns the box.
- Use bundler source maps (Vite, Next.js, Webpack 5 with `devtool: 'source-map'`) to map that component back to a `file:line` in the user's repo.
- Verify the mapping against the local git working tree (so we know if the file moved between capture and replay).
- Output: `{ component: "PriceLabel", file: "apps/web/src/features/checkout/PriceLabel.tsx", line: 42, framework: "react", confidence: 0.94 }`.

**This is the wedge.** Source maps for production bundles are fragile, but v1 only needs to work in **dev mode and Vercel/Netlify preview deploys** — which is where developers actually run the tool against bug reports.

### 3.3 Context payload (what the AI gets)

Structured JSON + a Markdown summary. No "synthesis." Just facts:

```json
{
  "annotation": {
    "shape": "rect",
    "coords": { "x": 140, "y": 310, "w": 280, "h": 60 },
    "note": "make this smaller and match the login button color"
  },
  "source": {
    "component": "PriceLabel",
    "file": "apps/web/src/features/checkout/PriceLabel.tsx",
    "line": 42,
    "framework": "react",
    "confidence": 0.94
  },
  "page": {
    "url": "http://localhost:3000/checkout",
    "title": "Checkout",
    "viewport": { "w": 1440, "h": 900 }
  },
  "console": [ /* last 50 lines */ ],
  "network": [ /* last 20 requests, headers + status only, no bodies */ ],
  "git": {
    "branch": "feat/checkout-redesign",
    "head": "a1b2c3d",
    "dirty": true
  }
}
```

The user's note ships verbatim. The IDE agent (Cursor, Claude Code) decides how to interpret "smaller." We don't pretend to know.

### 3.4 The handoff: MCP server, local-first

- Lightweight local MCP server bundled with the SDK or run as a desktop helper.
- Captures live in-memory; nothing leaves the developer's machine unless they explicitly share.
- IDE agent queries `glitch-spot://snapshots/latest` and gets the payload above.
- Same machine, same user — no auth ceremony, no link expiration, no passcode theatre.

For team sharing (next section): an authenticated workspace sync, not public links.

---

## 4. The user motion (the only one we optimize for)

1. Developer runs their app on localhost (or opens a Vercel preview).
2. Sees a visual bug. Hits `Cmd+Shift+G`.
3. Draws a rectangle. Types a one-line note.
4. Glitch Spot captures payload locally.
5. Switches to Cursor / Claude Code. Types: "Apply the latest Glitch Spot snapshot."
6. Agent reads MCP, sees `file: ...PriceLabel.tsx:42`, opens the file, makes the change.

**Time from bug → PR draft: under 60 seconds.** That's the demo, that's the pitch, that's the wedge.

---

## 5. Security model (v1)

- **No public shareable links.** Period.
- **No cloud storage of payloads by default.** Snapshots live in `~/.glitch-spot/` on the developer's machine.
- **Team sync (optional, opt-in):** OAuth against GitHub org. Snapshots sync to a workspace bucket scoped to that org. No public URLs ever — access is gated by GitHub org membership.
- **Network payload capture:** headers + status only by default. Bodies are off by default and require explicit per-project opt-in via SDK config (`captureNetworkBodies: true`). Document loudly that this is for dev/staging only.
- **Console capture:** last 50 lines, capped at 8KB total.
- **PII redaction:** *not promised*. Documentation tells users: don't run this against production with real customer data. v1 is a developer tool for dev/staging, not a customer support tool.

This is a much smaller security surface than the PRD describes, and it's defensible in an enterprise security review.

---

## 6. "Why now?" — the actual argument

The original PRD doesn't make this case. Here's the honest one:

1. **MCP adoption crossed a threshold.** Cursor, Claude Code, Windsurf, and Zed all support MCP as of late 2025 / early 2026. A tool whose output is "an MCP resource the agent already knows how to consume" has zero integration cost — that wasn't true a year ago.
2. **Source maps in modern bundlers are reliable enough.** Vite, Next.js 14+, and Webpack 5 produce maps accurate enough to reverse-resolve a DOM node to a source line with >90% confidence in dev mode. The infra exists; nobody has packaged it for this use case.
3. **Multimodal models are good enough that "what is shown" is no longer the bottleneck.** The bottleneck is "which file, which line, which component" — i.e., the deterministic mapping problem we're solving.

These three trends together open a 12–18 month window. Beyond that, Cursor/Claude Code will likely ship something similar natively.

---

## 7. The hardest unresolved question

**Why doesn't Cursor / Claude Code just ship this themselves?**

Honest answer: they probably will, within 12–18 months. The play here is:

- **Move first** and become the default verb ("just glitch-spot it") before they get there.
- **Stay framework-agnostic.** Cursor's version will likely be tied to Cursor. Ours works in any IDE that speaks MCP — including the ones that don't exist yet.
- **Accept that this might be an acquisition target, not a public-company outcome.** A clean, well-built primitive that Cursor/Anthropic/JetBrains might buy is a fine outcome, and one that requires we ship something genuinely good.

If we're not willing to be acquired and we're not willing to compete head-on with Cursor on their home turf, we shouldn't build this.

---

## 8. Open questions for Dmitry

- Do we agree that bounding-box-to-source-file is the wedge, or is there a different defensible primitive we're missing?
- Do we agree that mobile + desktop are wrong for v1, or is there a specific ICP for them I'm undervaluing?
- Are we comfortable with the "dev/staging only, no PII promises" positioning? It cuts a market segment (QA tooling for prod) but de-risks security review massively.
- Acquisition vs. independent-company framing — which one are we building toward? It changes a lot of downstream decisions (pricing, fundraising, how much we invest in the MCP server vs. tight IDE integrations).
- Is there a competitor I'm missing that already does the bounding-box-to-source-file mapping? Builder.io's Visual Copilot is closest; need to check whether they expose it as a standalone primitive or only inside their builder.

---

## 9. What this seed does NOT cover (yet)

- Pricing model
- Specific GTM / first 10 customers
- Engineering plan / staffing
- Detailed source-map reverse-mapping algorithm (this is where the technical risk lives and deserves its own design doc)
- Framework coverage matrix (React only? Vue? Svelte? what about RSC / Server Actions?)

These should be follow-up seeds or design docs once we agree on the thesis above.
