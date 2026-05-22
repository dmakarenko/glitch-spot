# Glitch Spot v0.2 — Drawing + Clipboard Save Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to execute this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.
>
> Part of initiative: [`docs/initiatives/glitch-spot/INITIATIVE.md`](../../initiatives/glitch-spot/INITIATIVE.md).
> Working scope reference: [`docs/initiatives/glitch-spot/scope.md`](../../initiatives/glitch-spot/scope.md).

**Goal:** Drag-to-draw rectangle annotations with inline notes on the overlay (in both live and frozen modes), undo, Save (`Cmd+Return`) / Discard (`Esc`-with-content) modal, and a save action that emits a structured snapshot JSON to the system clipboard alongside a clean PNG written to `/tmp/glitch-spot-<id>.png`.

**Architecture:** Frontend is React with a `useReducer`-managed annotation store keyed off the overlay window; rectangles render as absolutely-positioned DOM divs and notes as floating textareas next to them. Mouse drag drives the reducer; keyboard shortcuts (`Cmd+Z`, `Cmd+Return`, `Esc`) and a floating top-center toolbar drive the session-end actions. On save the React side invokes a new Tauri command `save_snapshot` with the annotation array; Rust assembles the snapshot, captures (in live mode) or reuses the cached frame (in frozen mode), writes `clean.png` to `/tmp`, serializes the JSON, copies it to the system clipboard via `tauri-plugin-clipboard-manager`, and hides the overlay.

**Tech Stack:** Tauri 2, Rust (existing), React 19 + TypeScript (existing), Vitest + @testing-library/react (new, for the reducer + small UI tests), `tauri-plugin-clipboard-manager` (new), `tauri-plugin-os` (new — for `os.platform()` / `os.version()` context fields), `uuid` Rust crate (new — snapshot id).

---

## Scope

**In scope:**

1. Drag-to-draw rectangles on the overlay in both live and frozen modes.
2. One inline floating note input per rectangle (multi-line `<textarea>`, plain text, ~500 chars).
3. Click an "X" on a rectangle (visible on hover) to delete it.
4. `Cmd+Z` undoes the last reversible action (rect create, note edit, rect delete).
5. Floating top-center toolbar showing the rectangle count and a "Save" button.
6. `Cmd+Return` or the Save button: serialize a snapshot, copy to clipboard, hide overlay.
7. `Esc` with no rectangles drawn → immediate dismiss (existing behavior preserved).
8. `Esc` with at least one rectangle → discard/save confirmation modal.
9. Snapshot JSON schema (subset of scope §6 — see §"Snapshot schema" below).
10. Clean PNG written to `/tmp/glitch-spot-<snapshot-id>.png`; JSON references it by absolute path.
11. Vitest + a small set of pure-logic tests covering the reducer and the Rust snapshot serializer.

**Explicitly out of scope (deferred):**

- Persistence to `~/.glitch-spot/snapshots/` — v0.3.
- Annotated PNG generation (rectangles + note text composited on the clean PNG) — addressed in the v0.2 follow-up plan [`2026-05-22-glitch-spot-v0.2-annotated-png.md`](./2026-05-22-glitch-spot-v0.2-annotated-png.md).
- Multi-select, drag, or resize rectangles — later.
- Free-form shapes, arrows, brushes — later.
- MCP server — v0.4.
- Settings UI / hotkey config — later.
- Color palette / annotation styling — later (single hard-coded color for v0.2).
- Image-in-clipboard (vs JSON-in-clipboard) — out; only JSON in v0.2.

---

## Snapshot schema (v0.2)

JSON copied to clipboard on save. Concrete shape, locked here so the Rust types in M1 and the React serialization in M11 stay aligned.

```json
{
  "id": "snap_01HXYZ...",
  "created_at": "2026-05-22T14:32:11Z",
  "mode": "frozen",
  "annotations": [
    {
      "id": "anno_1",
      "shape": "rect",
      "coords": { "x": 280, "y": 620, "w": 560, "h": 120 },
      "note": "make this smaller and match the login button color"
    }
  ],
  "context": {
    "screen": { "width": 2560, "height": 1414, "scale_factor": 2.0, "display_index": 0 },
    "os": { "platform": "macos", "version": "26.0" }
  },
  "images": {
    "clean": "/tmp/glitch-spot-snap_01HXYZ.png",
    "annotated": null
  }
}
```

**Coordinate space:** all `coords` are in the screenshot's **physical pixel space** (i.e., scale_factor already applied), with origin at the top-left of the captured work-area image. A downstream consumer (agent) can use them directly against the `images.clean` file.

**The React reducer stores coords in CSS pixels** during the session and multiplies by `scale_factor` only at serialization time. This keeps mouse-event math simple.

**`mode`:** `"live"` or `"frozen"` — communicates which hotkey opened the overlay (snapshot is identical in shape either way, but agents can use this hint).

**`images.annotated`** is `null` for v0.2. v0.3 fills it.

---

## Verification mode by milestone

| Milestone | Mode | Why |
|---|---|---|
| M1 Snapshot types (Rust) | TDD (cargo test) | Pure data + serde |
| M2 Vitest setup | Manual smoke | "test passes" baseline |
| M3 Reducer | TDD (vitest) | Pure data |
| M4 Drawing layer | Manual visual | DOM event handling |
| M5 Note inputs | Manual visual | Auto-positioning has to be eyeballed |
| M6 Delete-on-hover | Manual visual | UI affordance |
| M7 Keyboard shortcuts | Mixed: TDD reducer-binding, manual key dispatch | Key-event handlers don't unit-test cleanly |
| M8 Toolbar | Manual visual | Pure presentation |
| M9 Discard modal | Manual visual | Modal flow |
| M10 Save command | TDD JSON output; manual clipboard | Clipboard side effect can't be unit-tested in headless |
| M11 Wire-up | Manual e2e | Cross-component interaction |
| M12 Acceptance | Manual checklist | End-to-end |

---

## File structure

**Files created:**

```
src/overlay/annotations/
├── types.ts                          # shared types (Rect, Annotation, SaveRequest)
├── state.ts                          # useReducer state + actions + selectors
├── state.test.ts                     # reducer tests
├── DrawingLayer.tsx                  # mouse drag + rectangle rendering
├── RectangleNote.tsx                 # floating textarea anchored to a rect
└── DiscardModal.tsx                  # confirm modal on Esc-with-content

src/overlay/Toolbar.tsx               # floating top-center toolbar

src-tauri/src/snapshot/
├── mod.rs                            # types + serialize + clipboard write
└── tests.rs                          # JSON round-trip + shape assertions

vitest.config.ts                      # vitest config
```

**Files modified:**

```
src/overlay/Overlay.tsx               # wire DrawingLayer + Toolbar + modal + keyboard
src/App.tsx                           # add "Save (debug)" button that prints clipboard JSON
src-tauri/src/lib.rs                  # register clipboard plugin + save_snapshot command
src-tauri/Cargo.toml                  # add deps
package.json                          # add devDeps + test script
```

---

## Milestones and tasks

### M1: Rust snapshot types + serialization

#### Task 1.1: Add deps and define the snapshot module skeleton

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/snapshot/mod.rs`, `src-tauri/src/snapshot/tests.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `uuid`, `chrono`, and `tauri-plugin-clipboard-manager` to `src-tauri/Cargo.toml` `[dependencies]`:**

```toml
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", default-features = false, features = ["clock", "serde", "std"] }
tauri-plugin-clipboard-manager = "2"
tauri-plugin-os = "2"
```

(Pin versions to whatever the latest stable is at impl time. The `default-features = false` + `clock` keeps chrono lean — we only need UTC `now`.)

- [ ] **Step 2: Create `src-tauri/src/snapshot/mod.rs`:**

```rust
// ABOUTME: Snapshot data model + JSON serialization for the v0.2 save action.
// ABOUTME: Coords are physical pixels relative to the captured work-area image.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub id: String,
    pub created_at: String,
    pub mode: String,
    pub annotations: Vec<Annotation>,
    pub context: Context,
    pub images: Images,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    pub id: String,
    pub shape: String,
    pub coords: Coords,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Coords {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Context {
    pub screen: ScreenInfo,
    pub os: OsInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScreenInfo {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub display_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OsInfo {
    pub platform: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Images {
    pub clean: String,
    pub annotated: Option<String>,
}

pub fn new_id() -> String {
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    format!("snap_{}", &uuid[..12])
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 3: Create `src-tauri/src/snapshot/tests.rs`:**

```rust
// ABOUTME: Snapshot serde round-trip and shape contract tests.

use super::*;

fn sample_snapshot() -> Snapshot {
    Snapshot {
        id: "snap_test123".into(),
        created_at: "2026-05-22T14:32:11Z".into(),
        mode: "frozen".into(),
        annotations: vec![Annotation {
            id: "anno_1".into(),
            shape: "rect".into(),
            coords: Coords { x: 280.0, y: 620.0, w: 560.0, h: 120.0 },
            note: "make this smaller".into(),
        }],
        context: Context {
            screen: ScreenInfo { width: 2560, height: 1414, scale_factor: 2.0, display_index: 0 },
            os: OsInfo { platform: "macos".into(), version: "26.0".into() },
        },
        images: Images {
            clean: "/tmp/glitch-spot-snap_test123.png".into(),
            annotated: None,
        },
    }
}

#[test]
fn round_trips_through_json() {
    let s = sample_snapshot();
    let json = serde_json::to_string(&s).unwrap();
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[test]
fn json_uses_snake_case_keys() {
    let s = sample_snapshot();
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"created_at\""), "expected snake_case created_at, got {json}");
    assert!(json.contains("\"scale_factor\""));
    assert!(json.contains("\"display_index\""));
}

#[test]
fn annotated_serializes_as_null_when_none() {
    let s = sample_snapshot();
    let json = serde_json::to_string(&s).unwrap();
    assert!(
        json.contains("\"annotated\":null"),
        "expected explicit null for annotated, got {json}"
    );
}

#[test]
fn new_id_has_snap_prefix_and_reasonable_length() {
    let id = new_id();
    assert!(id.starts_with("snap_"));
    assert_eq!(id.len(), "snap_".len() + 12);
}
```

- [ ] **Step 4: Register the module in `src-tauri/src/lib.rs`.**

Edit `src-tauri/src/lib.rs` and add `mod snapshot;` near the other `mod` lines:

```rust
mod capture;
mod hotkey;
mod overlay;
mod snapshot;
mod spike_capture;
```

- [ ] **Step 5: Run the tests and confirm green:**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml --lib snapshot 2>&1 | tail -10
```

Expected: 4 passed; 0 failed.

- [ ] **Step 6: Commit:**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/snapshot/ src-tauri/src/lib.rs
git commit -m "Add Snapshot type and JSON contract tests"
```

---

### M2: Vitest setup for frontend tests

#### Task 2.1: Install vitest + add config + verify

**Files:**
- Modify: `package.json`
- Create: `vitest.config.ts`, `src/overlay/annotations/state.test.ts` (placeholder for M3)

- [ ] **Step 1: Install devDeps:**

```bash
npm install --save-dev vitest @testing-library/react @testing-library/jest-dom jsdom
```

- [ ] **Step 2: Create `vitest.config.ts`:**

```ts
// ABOUTME: Vitest config — jsdom env so React tests work, no separate build pipeline.
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: false,
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
```

- [ ] **Step 3: Add a `test` script to `package.json`:**

Edit `package.json` `scripts` to include:

```json
"test": "vitest run",
"test:watch": "vitest"
```

- [ ] **Step 4: Add a sanity test at `src/overlay/annotations/state.test.ts` (replaced fully in M3):**

```ts
// ABOUTME: Placeholder so vitest has something to run before M3 fills it in.
import { describe, expect, it } from "vitest";

describe("vitest is wired", () => {
  it("runs", () => {
    expect(1 + 1).toBe(2);
  });
});
```

(Create `src/overlay/annotations/` directory if missing.)

- [ ] **Step 5: Run the test:**

```bash
npm test 2>&1 | tail -10
```

Expected: `1 passed`, exit 0.

- [ ] **Step 6: Commit:**

```bash
git add package.json package-lock.json vitest.config.ts src/overlay/annotations/state.test.ts
git commit -m "Set up vitest with jsdom for frontend tests"
```

---

### M3: Annotation state reducer (TDD)

#### Task 3.1: Types + initial state + ADD_RECT action

**Files:**
- Create: `src/overlay/annotations/types.ts`, `src/overlay/annotations/state.ts`
- Modify: `src/overlay/annotations/state.test.ts`

- [ ] **Step 1: Create `src/overlay/annotations/types.ts`:**

```ts
// ABOUTME: Shared annotation types for the overlay's drawing layer + Rust IPC payload.

export type Rect = {
  id: string;
  // CSS-pixel coordinates inside the overlay's content area.
  x: number;
  y: number;
  w: number;
  h: number;
  note: string;
};

export type AnnotationsState = {
  rects: Rect[];
  // Stack of reversible actions; the head is the most recent.
  history: HistoryEntry[];
  // The id of the rectangle currently being dragged into existence, if any.
  draggingId: string | null;
};

export type HistoryEntry =
  | { kind: "create"; id: string }
  | { kind: "delete"; rect: Rect; index: number }
  | { kind: "noteEdit"; id: string; previousNote: string };

export type AnnotationsAction =
  | { type: "BEGIN_DRAG"; id: string; x: number; y: number }
  | { type: "DRAG_TO"; x: number; y: number }
  | { type: "END_DRAG" }
  | { type: "SET_NOTE"; id: string; note: string }
  | { type: "DELETE_RECT"; id: string }
  | { type: "UNDO" }
  | { type: "CLEAR" };
```

- [ ] **Step 2: Write the failing tests in `src/overlay/annotations/state.test.ts` (replacing the placeholder):**

```ts
// ABOUTME: Reducer tests for the overlay's annotation state. Pure data, no DOM.
import { describe, expect, it } from "vitest";
import { reducer, initialState } from "./state";

describe("annotations reducer", () => {
  it("starts with no rects, no history, no drag", () => {
    expect(initialState).toEqual({ rects: [], history: [], draggingId: null });
  });

  it("BEGIN_DRAG creates a zero-size rect anchored at the start point", () => {
    const s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 100, y: 50 });
    expect(s.rects).toEqual([{ id: "r1", x: 100, y: 50, w: 0, h: 0, note: "" }]);
    expect(s.draggingId).toBe("r1");
    expect(s.history).toEqual([{ kind: "create", id: "r1" }]);
  });

  it("DRAG_TO updates the dragging rect's bottom-right while drag is active", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 100, y: 50 });
    s = reducer(s, { type: "DRAG_TO", x: 200, y: 100 });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 50, w: 100, h: 50 });
  });

  it("DRAG_TO handles drags up and to the left by flipping origin", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 200, y: 100 });
    s = reducer(s, { type: "DRAG_TO", x: 100, y: 50 });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 50, w: 100, h: 50 });
  });

  it("END_DRAG clears draggingId without removing the rect", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 100, y: 50 });
    s = reducer(s, { type: "DRAG_TO", x: 200, y: 100 });
    s = reducer(s, { type: "END_DRAG" });
    expect(s.draggingId).toBeNull();
    expect(s.rects).toHaveLength(1);
  });

  it("DRAG_TO is a no-op when there is no active drag", () => {
    const s = reducer(initialState, { type: "DRAG_TO", x: 10, y: 10 });
    expect(s).toEqual(initialState);
  });
});
```

- [ ] **Step 3: Run the test, confirm it fails because `state.ts` doesn't exist yet:**

```bash
npm test 2>&1 | tail -10
```

Expected: failure mentioning that `./state` cannot be resolved.

- [ ] **Step 4: Create `src/overlay/annotations/state.ts` with just what these tests need:**

```ts
// ABOUTME: useReducer state for the overlay's annotation layer. No React deps here —
// ABOUTME: keep it pure so the logic stays testable in plain Node/vitest.

import type {
  AnnotationsAction,
  AnnotationsState,
  HistoryEntry,
  Rect,
} from "./types";

export const initialState: AnnotationsState = {
  rects: [],
  history: [],
  draggingId: null,
};

export function reducer(
  state: AnnotationsState,
  action: AnnotationsAction
): AnnotationsState {
  switch (action.type) {
    case "BEGIN_DRAG": {
      const rect: Rect = {
        id: action.id,
        x: action.x,
        y: action.y,
        w: 0,
        h: 0,
        note: "",
      };
      const history: HistoryEntry[] = [
        ...state.history,
        { kind: "create", id: action.id },
      ];
      return { rects: [...state.rects, rect], history, draggingId: action.id };
    }

    case "DRAG_TO": {
      if (!state.draggingId) return state;
      const rects = state.rects.map((r) =>
        r.id === state.draggingId ? extendTo(r, action.x, action.y) : r
      );
      return { ...state, rects };
    }

    case "END_DRAG":
      return { ...state, draggingId: null };

    default:
      return state;
  }
}

function extendTo(rect: Rect, x: number, y: number): Rect {
  // The drag start anchored the rect at (rect.x + rect.w, rect.y + rect.h)? No —
  // the original anchor was (rect.x, rect.y) at BEGIN_DRAG with w=h=0. To support
  // dragging up/left we recompute origin from min(anchor, cursor).
  const ax = rect.w === 0 && rect.h === 0 ? rect.x : rect.x; // anchor x stays — see below
  const ay = rect.h === 0 && rect.w === 0 ? rect.y : rect.y;
  // Store anchor in rect.x,y; w/h are signed offsets resolved at render time? No —
  // we want positive w/h. Recompute origin from anchor + cursor:
  const _ = { ax, ay };
  void _;
  // Simpler: keep `originX`/`originY` implicit via the FIRST BEGIN_DRAG position.
  // To support up/left drags we need that origin stable. But rect.x is overwritten
  // when drag flips. The cleanest implementation: stash the anchor on the rect.
  // For v0.2 we use a parallel anchor field — see Step 5 below.
  return rect;
}
```

> **STOP — the `extendTo` above is intentionally incomplete to surface the design choice in Step 5.** Don't try to make this version pass; move on to Step 5.

- [ ] **Step 5: Replace `state.ts` with a version that stores an explicit anchor per dragging rect.**

Update `src/overlay/annotations/types.ts` to extend `Rect` with optional `anchorX` / `anchorY` (only set while the rect is being dragged):

```ts
export type Rect = {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  note: string;
  // Drag anchor — present only while the rect is the dragging target. Cleared on END_DRAG.
  anchorX?: number;
  anchorY?: number;
};
```

Then rewrite `state.ts`:

```ts
// ABOUTME: useReducer state for the overlay's annotation layer. No React deps here —
// ABOUTME: keep it pure so the logic stays testable in plain Node/vitest.

import type {
  AnnotationsAction,
  AnnotationsState,
  HistoryEntry,
  Rect,
} from "./types";

export const initialState: AnnotationsState = {
  rects: [],
  history: [],
  draggingId: null,
};

export function reducer(
  state: AnnotationsState,
  action: AnnotationsAction
): AnnotationsState {
  switch (action.type) {
    case "BEGIN_DRAG": {
      const rect: Rect = {
        id: action.id,
        x: action.x,
        y: action.y,
        w: 0,
        h: 0,
        note: "",
        anchorX: action.x,
        anchorY: action.y,
      };
      const history: HistoryEntry[] = [
        ...state.history,
        { kind: "create", id: action.id },
      ];
      return { rects: [...state.rects, rect], history, draggingId: action.id };
    }

    case "DRAG_TO": {
      if (!state.draggingId) return state;
      const rects = state.rects.map((r) =>
        r.id === state.draggingId ? extendTo(r, action.x, action.y) : r
      );
      return { ...state, rects };
    }

    case "END_DRAG": {
      const rects = state.rects.map((r) =>
        r.id === state.draggingId ? stripAnchor(r) : r
      );
      return { ...state, rects, draggingId: null };
    }

    default:
      return state;
  }
}

function extendTo(rect: Rect, cursorX: number, cursorY: number): Rect {
  const ax = rect.anchorX ?? rect.x;
  const ay = rect.anchorY ?? rect.y;
  const x = Math.min(ax, cursorX);
  const y = Math.min(ay, cursorY);
  const w = Math.abs(cursorX - ax);
  const h = Math.abs(cursorY - ay);
  return { ...rect, x, y, w, h };
}

function stripAnchor(rect: Rect): Rect {
  const { anchorX: _ax, anchorY: _ay, ...rest } = rect;
  return rest;
}
```

- [ ] **Step 6: Run tests, confirm all green:**

```bash
npm test 2>&1 | tail -20
```

Expected: 6 passed, 0 failed.

- [ ] **Step 7: Commit:**

```bash
git add src/overlay/annotations/types.ts src/overlay/annotations/state.ts src/overlay/annotations/state.test.ts
git commit -m "Add annotations reducer with drag-to-draw rectangle creation"
```

#### Task 3.2: SET_NOTE, DELETE_RECT, CLEAR

**Files:**
- Modify: `src/overlay/annotations/state.ts`, `src/overlay/annotations/state.test.ts`

- [ ] **Step 1: Append failing tests:**

Append to `src/overlay/annotations/state.test.ts`:

```ts
describe("notes and deletion", () => {
  function drawn(id: string): import("./types").AnnotationsState {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id, x: 0, y: 0 });
    s = reducer(s, { type: "DRAG_TO", x: 100, y: 50 });
    return reducer(s, { type: "END_DRAG" });
  }

  it("SET_NOTE updates note text and records the previous value in history", () => {
    const s = reducer(drawn("r1"), { type: "SET_NOTE", id: "r1", note: "hello" });
    expect(s.rects[0].note).toBe("hello");
    const last = s.history[s.history.length - 1];
    expect(last).toEqual({ kind: "noteEdit", id: "r1", previousNote: "" });
  });

  it("repeated SET_NOTE on the same rect collapses history (no entry per keystroke)", () => {
    let s = reducer(drawn("r1"), { type: "SET_NOTE", id: "r1", note: "h" });
    s = reducer(s, { type: "SET_NOTE", id: "r1", note: "he" });
    s = reducer(s, { type: "SET_NOTE", id: "r1", note: "hel" });
    const edits = s.history.filter((h) => h.kind === "noteEdit" && h.id === "r1");
    expect(edits).toHaveLength(1);
    expect(edits[0]).toEqual({ kind: "noteEdit", id: "r1", previousNote: "" });
  });

  it("DELETE_RECT removes the rect and records it in history with its original index", () => {
    let s = drawn("r1");
    s = reducer(s, { type: "BEGIN_DRAG", id: "r2", x: 200, y: 0 });
    s = reducer(s, { type: "END_DRAG" });
    s = reducer(s, { type: "DELETE_RECT", id: "r1" });
    expect(s.rects.map((r) => r.id)).toEqual(["r2"]);
    const last = s.history[s.history.length - 1];
    expect(last.kind).toBe("delete");
    if (last.kind === "delete") {
      expect(last.rect.id).toBe("r1");
      expect(last.index).toBe(0);
    }
  });

  it("CLEAR returns to initial state", () => {
    let s = drawn("r1");
    s = reducer(s, { type: "SET_NOTE", id: "r1", note: "hi" });
    s = reducer(s, { type: "CLEAR" });
    expect(s).toEqual(initialState);
  });
});
```

- [ ] **Step 2: Run tests, confirm they fail:**

```bash
npm test 2>&1 | tail -15
```

Expected: 4 new failures (SET_NOTE / DELETE_RECT / CLEAR aren't handled).

- [ ] **Step 3: Extend the reducer in `src/overlay/annotations/state.ts`. Add cases inside the `switch`:**

```ts
    case "SET_NOTE": {
      const rect = state.rects.find((r) => r.id === action.id);
      if (!rect) return state;
      const rects = state.rects.map((r) =>
        r.id === action.id ? { ...r, note: action.note } : r
      );
      const alreadyEditing = state.history.some(
        (h) => h.kind === "noteEdit" && h.id === action.id
      );
      const history: HistoryEntry[] = alreadyEditing
        ? state.history
        : [...state.history, { kind: "noteEdit", id: action.id, previousNote: rect.note }];
      return { ...state, rects, history };
    }

    case "DELETE_RECT": {
      const index = state.rects.findIndex((r) => r.id === action.id);
      if (index < 0) return state;
      const rect = state.rects[index];
      const rects = state.rects.filter((r) => r.id !== action.id);
      const history: HistoryEntry[] = [...state.history, { kind: "delete", rect, index }];
      return { ...state, rects, history };
    }

    case "CLEAR":
      return initialState;
```

- [ ] **Step 4: Run tests, confirm green (10 total):**

```bash
npm test 2>&1 | tail -10
```

- [ ] **Step 5: Commit:**

```bash
git add src/overlay/annotations/state.ts src/overlay/annotations/state.test.ts
git commit -m "Add SET_NOTE, DELETE_RECT, CLEAR to annotations reducer"
```

#### Task 3.3: UNDO

**Files:**
- Modify: `src/overlay/annotations/state.ts`, `src/overlay/annotations/state.test.ts`

- [ ] **Step 1: Add failing tests:**

```ts
describe("undo", () => {
  function drawn(id: string, state = initialState): import("./types").AnnotationsState {
    let s = reducer(state, { type: "BEGIN_DRAG", id, x: 0, y: 0 });
    s = reducer(s, { type: "DRAG_TO", x: 100, y: 50 });
    return reducer(s, { type: "END_DRAG" });
  }

  it("UNDO on empty history is a no-op", () => {
    const s = reducer(initialState, { type: "UNDO" });
    expect(s).toEqual(initialState);
  });

  it("UNDO of a create removes the rect", () => {
    let s = drawn("r1");
    s = reducer(s, { type: "UNDO" });
    expect(s.rects).toEqual([]);
    expect(s.history).toEqual([]);
  });

  it("UNDO of a delete restores the rect at its original index", () => {
    let s = drawn("r1");
    s = drawn("r2", s);
    s = reducer(s, { type: "DELETE_RECT", id: "r1" });
    expect(s.rects.map((r) => r.id)).toEqual(["r2"]);
    s = reducer(s, { type: "UNDO" });
    expect(s.rects.map((r) => r.id)).toEqual(["r1", "r2"]);
  });

  it("UNDO of a note edit restores the prior note text", () => {
    let s = drawn("r1");
    s = reducer(s, { type: "SET_NOTE", id: "r1", note: "later" });
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0].note).toBe("");
  });
});
```

- [ ] **Step 2: Run tests, confirm failures.**

- [ ] **Step 3: Implement UNDO. Add to the reducer:**

```ts
    case "UNDO": {
      const history = [...state.history];
      const last = history.pop();
      if (!last) return state;
      switch (last.kind) {
        case "create":
          return {
            ...state,
            rects: state.rects.filter((r) => r.id !== last.id),
            history,
          };
        case "delete": {
          const rects = [...state.rects];
          const insertAt = Math.min(last.index, rects.length);
          rects.splice(insertAt, 0, last.rect);
          return { ...state, rects, history };
        }
        case "noteEdit": {
          const rects = state.rects.map((r) =>
            r.id === last.id ? { ...r, note: last.previousNote } : r
          );
          return { ...state, rects, history };
        }
      }
    }
```

- [ ] **Step 4: Run tests, confirm all 14 green.**

- [ ] **Step 5: Commit:**

```bash
git add src/overlay/annotations/state.ts src/overlay/annotations/state.test.ts
git commit -m "Add UNDO action to annotations reducer"
```

---

### M4: Drawing layer (mouse handling + rectangle rendering)

#### Task 4.1: DrawingLayer component

**Files:**
- Create: `src/overlay/annotations/DrawingLayer.tsx`

- [ ] **Step 1: Create the component:**

```tsx
// ABOUTME: Mouse-driven drawing layer. Click-drag to draw rectangles; renders all
// ABOUTME: rectangles as absolutely-positioned DOM divs the parent can wire to notes.

import { useCallback, useRef } from "react";
import type { AnnotationsAction, AnnotationsState } from "./types";

const RECT_BORDER = "2px solid #ff3b30";
const RECT_FILL = "rgba(255, 59, 48, 0.08)";

let _nextId = 1;
export function nextRectId(): string {
  return `r${_nextId++}`;
}

type Props = {
  state: AnnotationsState;
  dispatch: (action: AnnotationsAction) => void;
};

export function DrawingLayer({ state, dispatch }: Props) {
  const ref = useRef<HTMLDivElement | null>(null);

  const onMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      // Only respond to clicks that started on the drawing surface itself, not
      // on a child element (e.g., a note input or a delete X). Children stop
      // propagation via their own handlers.
      if (e.target !== e.currentTarget) return;
      // Ignore non-primary buttons.
      if (e.button !== 0) return;
      const rect = ref.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      dispatch({ type: "BEGIN_DRAG", id: nextRectId(), x, y });
    },
    [dispatch]
  );

  const onMouseMove = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!state.draggingId) return;
      const rect = ref.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      dispatch({ type: "DRAG_TO", x, y });
    },
    [dispatch, state.draggingId]
  );

  const onMouseUp = useCallback(() => {
    if (!state.draggingId) return;
    dispatch({ type: "END_DRAG" });
  }, [dispatch, state.draggingId]);

  return (
    <div
      ref={ref}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      onMouseLeave={onMouseUp}
      style={{
        position: "absolute",
        inset: 0,
        cursor: "crosshair",
      }}
    >
      {state.rects.map((r) => (
        <div
          key={r.id}
          data-rect-id={r.id}
          style={{
            position: "absolute",
            left: r.x,
            top: r.y,
            width: r.w,
            height: r.h,
            border: RECT_BORDER,
            background: RECT_FILL,
            pointerEvents: "none",
            boxSizing: "border-box",
          }}
        />
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Wire it into `src/overlay/Overlay.tsx`.**

Replace the existing render body of Overlay so DrawingLayer sits on top of the backdrop:

```tsx
import { useEffect, useReducer, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { DrawingLayer } from "./annotations/DrawingLayer";
import { initialState, reducer } from "./annotations/state";

// ...existing Frame / Mode / CHECKER_BG / frameToObjectUrl declarations stay...

export function Overlay() {
  const [mode, setMode] = useState<Mode>("live");
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [annotations, dispatch] = useReducer(reducer, initialState);

  useEffect(() => {
    // ...existing useEffect body (frame listener, __glitchSpotPresent, esc handler)...
    // For now keep the Esc handler invoking hide_overlay directly. M9 changes it.
  }, []);

  const onClick = (e: React.MouseEvent<HTMLDivElement>) => {
    // Suppress click-to-dismiss now that the surface is interactive. Use
    // the explicit save / Esc paths instead. M11 wires those.
    if (e.target === e.currentTarget) {
      /* intentionally no dismiss while drawing is active */
    }
  };

  const backdrop =
    mode === "frozen" && imageUrl
      ? `center / 100% 100% no-repeat url(${imageUrl})`
      : "transparent";

  return (
    <div
      onClick={onClick}
      style={{ position: "fixed", inset: 0, background: backdrop }}
    >
      <div
        style={{
          position: "absolute",
          inset: 0,
          backgroundImage: CHECKER_BG,
          backgroundRepeat: "repeat",
          opacity: 0.12,
          pointerEvents: "none",
        }}
      />
      <DrawingLayer state={annotations} dispatch={dispatch} />
    </div>
  );
}
```

- [ ] **Step 3: Verify the build still compiles:**

```bash
npx tsc --noEmit
```

Expected: exit 0.

- [ ] **Step 4: Manual visual verification (Dmitry's job — record the expectation here):**

Build + launch:

```bash
PATH="$HOME/.cargo/bin:$PATH" npm run tauri build 2>&1 | tail -3
DYLD_FALLBACK_LIBRARY_PATH=/usr/lib/swift src-tauri/target/release/glitch_spot &
```

Then:
- Press `Ctrl+Opt+Cmd+G`. Click-drag inside the overlay. A red rectangle outlines the drag area in real time.
- Release the mouse. The rectangle stays.
- Drag again. A second rectangle appears.
- Drag from bottom-right to top-left. Rectangle still grows correctly (no flipped axes).
- `Esc` still hides the overlay (existing behavior).

If any of these fail, STOP and report.

- [ ] **Step 5: Commit:**

```bash
git add src/overlay/annotations/DrawingLayer.tsx src/overlay/Overlay.tsx
git commit -m "Add DrawingLayer with drag-to-draw rectangles"
```

---

### M5: Per-rectangle floating note input

#### Task 5.1: RectangleNote component

**Files:**
- Create: `src/overlay/annotations/RectangleNote.tsx`
- Modify: `src/overlay/annotations/DrawingLayer.tsx`

- [ ] **Step 1: Create `src/overlay/annotations/RectangleNote.tsx`:**

```tsx
// ABOUTME: Floating textarea anchored to a rectangle. Auto-positions below the rect,
// ABOUTME: or above if it would overflow the viewport.

import { useEffect, useRef } from "react";
import type { Rect } from "./types";

const NOTE_WIDTH = 240;
const NOTE_OFFSET = 6;
const NOTE_MIN_HEIGHT = 56;

type Props = {
  rect: Rect;
  containerHeight: number;
  onChange: (note: string) => void;
  autoFocus: boolean;
};

export function RectangleNote({ rect, containerHeight, onChange, autoFocus }: Props) {
  const ref = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);

  // Auto-flip vertically if placing below would overflow.
  const belowY = rect.y + rect.h + NOTE_OFFSET;
  const wouldOverflowBelow = belowY + NOTE_MIN_HEIGHT > containerHeight;
  const top = wouldOverflowBelow ? rect.y - NOTE_MIN_HEIGHT - NOTE_OFFSET : belowY;

  // Keep within left/right bounds.
  const left = Math.max(0, Math.min(rect.x, window.innerWidth - NOTE_WIDTH));

  return (
    <textarea
      ref={ref}
      value={rect.note}
      onChange={(e) => onChange(e.target.value)}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      placeholder="Note (optional)"
      maxLength={500}
      rows={3}
      style={{
        position: "absolute",
        top,
        left,
        width: NOTE_WIDTH,
        minHeight: NOTE_MIN_HEIGHT,
        padding: "6px 8px",
        font: "13px/1.4 system-ui",
        background: "rgba(255,255,255,0.96)",
        color: "#111",
        border: "1px solid rgba(0,0,0,0.18)",
        borderRadius: 6,
        boxShadow: "0 4px 16px rgba(0,0,0,0.18)",
        outline: "none",
        resize: "none",
      }}
    />
  );
}
```

- [ ] **Step 2: Render notes from DrawingLayer.**

Modify `src/overlay/annotations/DrawingLayer.tsx`. Add an import:

```tsx
import { RectangleNote } from "./RectangleNote";
```

Track container height via a ref. Replace the inner JSX inside the outer `<div>`:

```tsx
const [containerHeight, setContainerHeight] = useState<number>(
  typeof window !== "undefined" ? window.innerHeight : 0
);

useEffect(() => {
  const update = () => setContainerHeight(window.innerHeight);
  window.addEventListener("resize", update);
  return () => window.removeEventListener("resize", update);
}, []);
```

(Add `useEffect`, `useState` to the React imports.)

In the JSX, after the rect rendering loop, add another loop:

```tsx
{state.rects.map((r) => (
  <RectangleNote
    key={`${r.id}-note`}
    rect={r}
    containerHeight={containerHeight}
    autoFocus={r.id === state.draggingId}
    onChange={(note) => dispatch({ type: "SET_NOTE", id: r.id, note })}
  />
))}
```

(The autoFocus uses `draggingId` — even though END_DRAG nulls it, we want focus when the user JUST released. We change this to a "lastCreatedId" tracker in Step 3.)

- [ ] **Step 3: Add `lastCreatedId` tracking so the note focuses right after `END_DRAG`.**

In `src/overlay/annotations/types.ts`, extend `AnnotationsState`:

```ts
export type AnnotationsState = {
  rects: Rect[];
  history: HistoryEntry[];
  draggingId: string | null;
  lastCreatedId: string | null;
};
```

In `src/overlay/annotations/state.ts`:

```ts
export const initialState: AnnotationsState = {
  rects: [],
  history: [],
  draggingId: null,
  lastCreatedId: null,
};
```

In the `BEGIN_DRAG` case, set `lastCreatedId: action.id` alongside `draggingId`.

In `CLEAR` and `UNDO of create`, also clear `lastCreatedId` when the lastCreated rect is removed.

Update the corresponding tests in `state.test.ts` to assert `lastCreatedId` is set on BEGIN_DRAG and cleared on CLEAR. Run `npm test` to confirm green.

- [ ] **Step 4: Update DrawingLayer's note loop to use `lastCreatedId`:**

```tsx
{state.rects.map((r) => (
  <RectangleNote
    key={`${r.id}-note`}
    rect={r}
    containerHeight={containerHeight}
    autoFocus={r.id === state.lastCreatedId}
    onChange={(note) => dispatch({ type: "SET_NOTE", id: r.id, note })}
  />
))}
```

- [ ] **Step 5: Visual verification:**

Rebuild + launch (as in M4 Step 4). Press hotkey, draw a rectangle, release. A textarea appears below the rectangle and the cursor is in it. Type — the text appears. Draw another — its textarea appears, focus moves to it. If a rect is drawn near the bottom of the screen, the textarea flips above.

- [ ] **Step 6: Commit:**

```bash
git add src/overlay/annotations/RectangleNote.tsx src/overlay/annotations/DrawingLayer.tsx src/overlay/annotations/types.ts src/overlay/annotations/state.ts src/overlay/annotations/state.test.ts
git commit -m "Add per-rectangle floating note input with auto-flip"
```

---

### M6: Delete-on-hover for rectangles

#### Task 6.1: Hover X button

**Files:**
- Modify: `src/overlay/annotations/DrawingLayer.tsx`

- [ ] **Step 1: Add a delete affordance inside the rectangle render loop.**

Replace the existing rectangle-rendering JSX with:

```tsx
{state.rects.map((r) => (
  <div
    key={r.id}
    data-rect-id={r.id}
    onMouseDown={(e) => e.stopPropagation()}
    style={{
      position: "absolute",
      left: r.x,
      top: r.y,
      width: r.w,
      height: r.h,
      border: RECT_BORDER,
      background: RECT_FILL,
      boxSizing: "border-box",
    }}
  >
    {r.w > 24 && r.h > 24 && (
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          dispatch({ type: "DELETE_RECT", id: r.id });
        }}
        onMouseDown={(e) => e.stopPropagation()}
        aria-label="Delete rectangle"
        style={{
          position: "absolute",
          top: -10,
          right: -10,
          width: 20,
          height: 20,
          padding: 0,
          border: "none",
          borderRadius: "50%",
          background: "#ff3b30",
          color: "white",
          fontSize: 14,
          lineHeight: "20px",
          textAlign: "center",
          cursor: "pointer",
          boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
        }}
      >
        ×
      </button>
    )}
  </div>
))}
```

Note that we removed `pointerEvents: "none"` from the rect div (it was there to keep clicks on the surface). Add it back conditionally — only the rect's BODY should pass through clicks, not the X button. We achieve that by setting `pointer-events: none` on the rect div and `pointer-events: auto` on the button.

Update accordingly:

```tsx
<div
  key={r.id}
  data-rect-id={r.id}
  style={{
    position: "absolute",
    left: r.x,
    top: r.y,
    width: r.w,
    height: r.h,
    border: RECT_BORDER,
    background: RECT_FILL,
    boxSizing: "border-box",
    pointerEvents: "none",  // rect body is decorative
  }}
>
  {r.w > 24 && r.h > 24 && (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        dispatch({ type: "DELETE_RECT", id: r.id });
      }}
      onMouseDown={(e) => e.stopPropagation()}
      aria-label="Delete rectangle"
      style={{
        position: "absolute",
        top: -10,
        right: -10,
        width: 20,
        height: 20,
        padding: 0,
        border: "none",
        borderRadius: "50%",
        background: "#ff3b30",
        color: "white",
        fontSize: 14,
        lineHeight: "20px",
        textAlign: "center",
        cursor: "pointer",
        pointerEvents: "auto",  // button is interactive
        boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
      }}
    >
      ×
    </button>
  )}
</div>
```

- [ ] **Step 2: Visual verification:**

Rebuild + launch. Draw a rectangle. A small × appears at its top-right. Click it — the rectangle vanishes. Tiny rectangles (<= 24×24) suppress the X to avoid overlap with the note input.

- [ ] **Step 3: Commit:**

```bash
git add src/overlay/annotations/DrawingLayer.tsx
git commit -m "Add hover X to delete rectangles"
```

---

### M7: Keyboard shortcuts (Cmd+Z undo)

#### Task 7.1: Cmd+Z handler

**Files:**
- Modify: `src/overlay/Overlay.tsx`

- [ ] **Step 1: Replace the existing keydown handler block in Overlay's `useEffect` so that Cmd+Z dispatches UNDO and Esc still calls hide_overlay (M9 changes Esc later):**

Inside the existing `useEffect`:

```tsx
const onKey = (e: KeyboardEvent) => {
  if (e.key === "Escape") {
    invoke("hide_overlay");
    return;
  }
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
    e.preventDefault();
    dispatch({ type: "UNDO" });
    return;
  }
};
window.addEventListener("keydown", onKey);
```

- [ ] **Step 2: Visual verification:**

Rebuild + launch. Draw two rectangles. Press `Cmd+Z` — the second rectangle disappears. Press `Cmd+Z` again — the first disappears. Press once more — no-op (history empty).

Edit a note, then press `Cmd+Z` — the note's text reverts to empty (the previous value at the start of editing).

- [ ] **Step 3: Commit:**

```bash
git add src/overlay/Overlay.tsx
git commit -m "Add Cmd+Z undo binding"
```

---

### M8: Floating toolbar

#### Task 8.1: Toolbar component

**Files:**
- Create: `src/overlay/Toolbar.tsx`
- Modify: `src/overlay/Overlay.tsx`

- [ ] **Step 1: Create `src/overlay/Toolbar.tsx`:**

```tsx
// ABOUTME: Top-center floating toolbar for the overlay session.
// ABOUTME: Shows the rect count and a Save button. Hides until at least one rect exists.

type Props = {
  rectCount: number;
  onSave: () => void;
};

export function Toolbar({ rectCount, onSave }: Props) {
  if (rectCount === 0) return null;
  return (
    <div
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      style={{
        position: "fixed",
        top: 12,
        left: "50%",
        transform: "translateX(-50%)",
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "8px 12px",
        background: "rgba(20,20,20,0.85)",
        color: "white",
        font: "13px/1 system-ui",
        borderRadius: 8,
        boxShadow: "0 4px 18px rgba(0,0,0,0.35)",
        pointerEvents: "auto",
        zIndex: 10,
      }}
    >
      <span>{rectCount} rectangle{rectCount === 1 ? "" : "s"}</span>
      <button
        type="button"
        onClick={onSave}
        style={{
          padding: "6px 12px",
          background: "#0a84ff",
          color: "white",
          border: "none",
          borderRadius: 4,
          fontWeight: 600,
          cursor: "pointer",
        }}
      >
        Save (⌘↩)
      </button>
    </div>
  );
}
```

- [ ] **Step 2: Render it from `Overlay.tsx`:**

Add the import and render after `<DrawingLayer ... />`:

```tsx
import { Toolbar } from "./Toolbar";
// ...
<Toolbar rectCount={annotations.rects.length} onSave={() => { /* M11 wires this */ }} />
```

- [ ] **Step 3: Visual verification:**

Rebuild + launch. With no rects, the toolbar is invisible. Draw one — it appears. Draw another — count updates to "2 rectangles."

- [ ] **Step 4: Commit:**

```bash
git add src/overlay/Toolbar.tsx src/overlay/Overlay.tsx
git commit -m "Add floating Save toolbar"
```

---

### M9: Discard modal (Esc with content)

#### Task 9.1: DiscardModal component + Esc routing

**Files:**
- Create: `src/overlay/annotations/DiscardModal.tsx`
- Modify: `src/overlay/Overlay.tsx`

- [ ] **Step 1: Create `src/overlay/annotations/DiscardModal.tsx`:**

```tsx
// ABOUTME: Confirm modal shown when Esc is pressed with at least one rectangle drawn.
// ABOUTME: Lets the user discard the session or save it.

type Props = {
  onDiscard: () => void;
  onSave: () => void;
  onCancel: () => void;
};

export function DiscardModal({ onDiscard, onSave, onCancel }: Props) {
  return (
    <div
      onMouseDown={(e) => {
        // Click outside the dialog cancels.
        if (e.target === e.currentTarget) onCancel();
      }}
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.35)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 100,
      }}
    >
      <div
        style={{
          background: "white",
          color: "#111",
          minWidth: 320,
          padding: "20px 22px",
          borderRadius: 10,
          boxShadow: "0 16px 48px rgba(0,0,0,0.4)",
          font: "14px/1.5 system-ui",
        }}
      >
        <h2 style={{ margin: "0 0 8px", font: "600 16px/1.3 system-ui" }}>
          Discard annotations?
        </h2>
        <p style={{ margin: "0 0 16px", color: "#555" }}>
          You have unsaved rectangles. Save them to the clipboard, or discard?
        </p>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button
            type="button"
            onClick={onCancel}
            style={btn("transparent", "#444")}
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onDiscard}
            style={btn("#fff", "#b00020", "1px solid #b00020")}
          >
            Discard
          </button>
          <button
            type="button"
            onClick={onSave}
            style={btn("#0a84ff", "white")}
          >
            Save (⌘↩)
          </button>
        </div>
      </div>
    </div>
  );
}

function btn(bg: string, color: string, border = "none"): React.CSSProperties {
  return {
    padding: "6px 12px",
    background: bg,
    color,
    border,
    borderRadius: 4,
    cursor: "pointer",
    font: "13px/1 system-ui",
  };
}
```

- [ ] **Step 2: Wire Esc handling.**

In `src/overlay/Overlay.tsx`, add state for the modal:

```tsx
const [showDiscardModal, setShowDiscardModal] = useState(false);
```

Replace the existing Esc handler inside `onKey`:

```tsx
if (e.key === "Escape") {
  e.preventDefault();
  if (annotations.rects.length === 0) {
    invoke("hide_overlay");
  } else {
    setShowDiscardModal(true);
  }
  return;
}
```

> **Note:** the closure captures `annotations` at registration time. Move the keydown listener INSIDE an effect that depends on `annotations.rects.length`:

```tsx
useEffect(() => {
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      if (annotations.rects.length === 0) {
        invoke("hide_overlay");
      } else {
        setShowDiscardModal(true);
      }
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
      e.preventDefault();
      dispatch({ type: "UNDO" });
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}, [annotations.rects.length]);
```

Move the rest of the existing useEffect (frame listener, `__glitchSpotPresent`) into a SEPARATE useEffect with `[]` deps, since it's mount-once setup.

- [ ] **Step 3: Render the modal.**

```tsx
import { DiscardModal } from "./annotations/DiscardModal";
// ...
{showDiscardModal && (
  <DiscardModal
    onDiscard={() => {
      setShowDiscardModal(false);
      dispatch({ type: "CLEAR" });
      invoke("hide_overlay");
    }}
    onSave={() => { setShowDiscardModal(false); /* M11 wires the save flow */ }}
    onCancel={() => setShowDiscardModal(false)}
  />
)}
```

- [ ] **Step 4: Visual verification:**

Draw a rectangle, press Esc. Modal appears. Click Cancel → modal closes, rect remains. Press Esc again → modal reappears. Click Discard → modal closes, overlay hides, all rects gone. (Save doesn't do anything useful until M11.)

- [ ] **Step 5: Commit:**

```bash
git add src/overlay/annotations/DiscardModal.tsx src/overlay/Overlay.tsx
git commit -m "Add discard-or-save modal on Esc with content"
```

---

### M10: Snapshot save (backend)

#### Task 10.1: Snapshot assembly + clipboard write

**Files:**
- Modify: `src-tauri/src/snapshot/mod.rs` (add helper + extend tests)
- Create: `src-tauri/src/snapshot/build.rs` (assembly logic)
- Modify: `src-tauri/src/lib.rs` (register plugins, add command)

- [ ] **Step 1: Add `tauri-plugin-clipboard-manager` to capability allowlist.**

Edit `src-tauri/capabilities/default.json` to include the clipboard plugin's write permission:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default permissions",
  "windows": ["main", "overlay"],
  "permissions": [
    "core:default",
    "global-shortcut:default",
    "clipboard-manager:allow-write-text",
    "os:default"
  ]
}
```

(The exact permission name may be `clipboard-manager:allow-write` in newer plugin versions; consult `cargo doc` or the plugin's README if `cargo check` rejects the above. The build script validates capabilities, so any mismatch surfaces immediately.)

- [ ] **Step 2: Register the plugins in `src-tauri/src/lib.rs`.**

Inside `run()`, add to the Builder chain:

```rust
.plugin(tauri_plugin_clipboard_manager::init())
.plugin(tauri_plugin_os::init())
```

- [ ] **Step 3: Create `src-tauri/src/snapshot/build.rs`:**

```rust
// ABOUTME: Assembles a Snapshot from incoming annotation data plus the latest captured frame.
// ABOUTME: Writes the clean PNG to /tmp for v0.2; persistence to ~/.glitch-spot is v0.3 work.

use std::path::PathBuf;

use chrono::Utc;

use crate::capture::CapturedFrame;
use crate::snapshot::{
    new_id, Annotation, Context, Coords, Images, OsInfo, ScreenInfo, Snapshot,
};

/// One annotation as received from the React side. Coordinates are CSS pixels
/// relative to the overlay's content area; we multiply by `scale_factor` to
/// get physical pixels for the on-disk image.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct IncomingAnnotation {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub note: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("no captured frame available to save")]
    NoFrame,
}

/// Build a Snapshot, write the clean PNG to /tmp, and return both.
pub fn build_and_write(
    frame: &CapturedFrame,
    mode: &str,
    annotations: Vec<IncomingAnnotation>,
    os_platform: &str,
    os_version: &str,
) -> Result<Snapshot, BuildError> {
    let id = new_id();
    let clean_path = PathBuf::from(format!("/tmp/glitch-spot-{}.png", id));
    std::fs::write(&clean_path, &frame.png)?;

    let scale = frame.scale_factor;
    let annotations = annotations
        .into_iter()
        .map(|a| Annotation {
            id: a.id,
            shape: "rect".into(),
            coords: Coords {
                x: a.x * scale,
                y: a.y * scale,
                w: a.w * scale,
                h: a.h * scale,
            },
            note: a.note,
        })
        .collect();

    let snapshot = Snapshot {
        id,
        created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        mode: mode.to_string(),
        annotations,
        context: Context {
            screen: ScreenInfo {
                width: frame.width,
                height: frame.height,
                scale_factor: frame.scale_factor,
                display_index: frame.display_index,
            },
            os: OsInfo {
                platform: os_platform.to_string(),
                version: os_version.to_string(),
            },
        },
        images: Images {
            clean: clean_path.to_string_lossy().into_owned(),
            annotated: None,
        },
    };
    Ok(snapshot)
}
```

Add `mod build;` to `src-tauri/src/snapshot/mod.rs`:

```rust
pub mod build;

// ...rest of mod.rs unchanged...
```

- [ ] **Step 4: Add a unit test for `build_and_write` that asserts coordinate scaling.**

Append to `src-tauri/src/snapshot/tests.rs`:

```rust
use crate::capture::CapturedFrame;
use crate::snapshot::build::{build_and_write, IncomingAnnotation};

fn fake_frame() -> CapturedFrame {
    // A minimal valid PNG: 1x1 transparent pixel.
    let png: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
        0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41,
        0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
        0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
        0x42, 0x60, 0x82,
    ];
    CapturedFrame {
        width: 2,
        height: 2,
        scale_factor: 2.0,
        png,
        display_index: 0,
    }
}

#[test]
fn build_scales_annotation_coords_by_scale_factor() {
    let snap = build_and_write(
        &fake_frame(),
        "frozen",
        vec![IncomingAnnotation {
            id: "anno_x".into(),
            x: 10.0,
            y: 20.0,
            w: 30.0,
            h: 40.0,
            note: "hi".into(),
        }],
        "macos",
        "26.0",
    )
    .unwrap();
    assert_eq!(snap.annotations.len(), 1);
    let a = &snap.annotations[0];
    assert_eq!(a.coords.x, 20.0);
    assert_eq!(a.coords.y, 40.0);
    assert_eq!(a.coords.w, 60.0);
    assert_eq!(a.coords.h, 80.0);
}

#[test]
fn build_writes_png_to_tmp() {
    let snap = build_and_write(&fake_frame(), "live", vec![], "macos", "26.0").unwrap();
    let path = std::path::Path::new(&snap.images.clean);
    assert!(path.starts_with("/tmp/"));
    assert!(path.exists());
    let bytes = std::fs::read(path).unwrap();
    assert!(bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "must be a PNG");
    // Cleanup so we don't accumulate /tmp files when re-running tests.
    let _ = std::fs::remove_file(path);
}
```

- [ ] **Step 5: Run tests:**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml --lib snapshot 2>&1 | tail -15
```

Expected: all snapshot tests green (6 total: 4 from M1 + 2 new).

- [ ] **Step 6: Add the `save_snapshot` Tauri command in `src-tauri/src/lib.rs`:**

```rust
use snapshot::build::IncomingAnnotation;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_os as os;

#[tauri::command]
async fn save_snapshot(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
    annotations: Vec<IncomingAnnotation>,
) -> Result<snapshot::Snapshot, String> {
    let frame = match mode.as_str() {
        "frozen" => state
            .latest_frame
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .ok_or_else(|| "no cached frame for frozen mode".to_string())?,
        "live" => capture::capture_active_monitor(&app).map_err(|e| e.to_string())?,
        other => return Err(format!("unknown mode: {other}")),
    };

    let snap = snapshot::build::build_and_write(
        &frame,
        &mode,
        annotations,
        os::platform(),
        &os::version().to_string(),
    )
    .map_err(|e| e.to_string())?;

    let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
    app.clipboard()
        .write_text(json)
        .map_err(|e| format!("clipboard: {e}"))?;

    overlay::hide(&app).map_err(|e| e.to_string())?;
    Ok(snap)
}
```

Register it in `generate_handler!`:

```rust
.invoke_handler(tauri::generate_handler![
    capture_active_monitor,
    open_overlay_live,
    open_overlay_frozen,
    hide_overlay,
    get_latest_frame,
    save_snapshot,
    spike_capture::spike_capture,
])
```

- [ ] **Step 7: Verify the Rust side compiles and lints:**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -3
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --manifest-path src-tauri/Cargo.toml --no-deps -- -D warnings 2>&1 | tail -3
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check --manifest-path src-tauri/Cargo.toml
```

All three exit 0.

- [ ] **Step 8: Commit:**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/snapshot/ src-tauri/capabilities/default.json
git commit -m "Add save_snapshot command writing /tmp PNG and clipboard JSON"
```

---

### M11: Wire the save flow into the React side

#### Task 11.1: Trigger save from Cmd+Return, the toolbar, and the modal

**Files:**
- Modify: `src/overlay/Overlay.tsx`

- [ ] **Step 1: Add a `handleSave` async helper inside Overlay:**

```tsx
const handleSave = async () => {
  const payload = annotations.rects.map((r) => ({
    id: r.id,
    x: r.x,
    y: r.y,
    w: r.w,
    h: r.h,
    note: r.note,
  }));
  try {
    await invoke("save_snapshot", { mode, annotations: payload });
    // hide_overlay is invoked by Rust; React state will reset on next show.
    dispatch({ type: "CLEAR" });
  } catch (err) {
    console.error("save_snapshot failed", err);
  }
};
```

- [ ] **Step 2: Wire `Cmd+Return` into the same keydown effect:**

```tsx
if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
  e.preventDefault();
  if (annotations.rects.length > 0) handleSave();
}
```

Move the body of `handleSave` into a `useCallback` so the effect dep is stable, or capture the dispatch + annotations via the closure (matching the effect dep `[annotations.rects.length]` is fine since `dispatch` is stable and the rects array is only read when triggering save).

A practical version:

```tsx
useEffect(() => {
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      if (annotations.rects.length === 0) invoke("hide_overlay");
      else setShowDiscardModal(true);
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
      e.preventDefault();
      dispatch({ type: "UNDO" });
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      if (annotations.rects.length > 0) handleSave();
      return;
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}, [annotations.rects, mode]);
```

(`handleSave` is hoisted in scope; it depends on `annotations.rects` and `mode` which are deps.)

- [ ] **Step 3: Wire the Toolbar's `onSave` and the modal's `onSave`:**

```tsx
<Toolbar rectCount={annotations.rects.length} onSave={handleSave} />
```

```tsx
<DiscardModal
  onDiscard={() => {
    setShowDiscardModal(false);
    dispatch({ type: "CLEAR" });
    invoke("hide_overlay");
  }}
  onSave={() => { setShowDiscardModal(false); handleSave(); }}
  onCancel={() => setShowDiscardModal(false)}
/>
```

- [ ] **Step 4: Reset annotation state when the overlay is re-shown.**

The `__glitchSpotPresent` callback already clears imageUrl and sets mode. Extend it to clear annotations too. In the existing useEffect that registers it:

```tsx
(window as unknown as { __glitchSpotPresent?: (mode: Mode) => void })
  .__glitchSpotPresent = (newMode) => {
  if (currentUrl) URL.revokeObjectURL(currentUrl);
  currentUrl = null;
  setImageUrl(null);
  setMode(newMode);
  dispatch({ type: "CLEAR" });
  setShowDiscardModal(false);
};
```

- [ ] **Step 5: Verify typechecks pass:**

```bash
npx tsc --noEmit
```

Expected: exit 0.

- [ ] **Step 6: Commit:**

```bash
git add src/overlay/Overlay.tsx
git commit -m "Wire Cmd+Return, toolbar Save, and discard-modal Save to save_snapshot"
```

---

### M12: End-to-end acceptance

#### Task 12.1: Full manual walkthrough

- [ ] **Step 1: Build:**

```bash
pkill -KILL -f glitch_spot 2>/dev/null
PATH="$HOME/.cargo/bin:$PATH" npm run tauri build 2>&1 | tail -3
```

- [ ] **Step 2: Launch:**

```bash
DYLD_FALLBACK_LIBRARY_PATH=/usr/lib/swift src-tauri/target/release/glitch_spot &
```

- [ ] **Step 3: Walk the checklist (Dmitry runs each, fix anything that fails before declaring done):**

**Live mode (Ctrl+Opt+Cmd+G):**
- [ ] Overlay appears transparent, checker visible.
- [ ] Click-drag draws a rectangle.
- [ ] A note input appears below the rect; cursor is focused there.
- [ ] Type text. Press Cmd+Z. Text reverts to empty.
- [ ] Draw a second rect. Toolbar shows "2 rectangles."
- [ ] Click the × on the first rect. It disappears. Toolbar shows "1 rectangle."
- [ ] Click "Save (⌘↩)" on the toolbar. Overlay dismisses.
- [ ] In a text editor, paste. JSON appears with annotation coords, mode="live", images.clean=/tmp/glitch-spot-snap_...png.
- [ ] `open` the clean PNG path. The image shows the live desktop captured at save time.

**Frozen mode (Ctrl+Opt+Cmd+H):**
- [ ] Overlay appears with the captured screenshot as backdrop.
- [ ] Draw a rectangle around a UI element.
- [ ] Type a note.
- [ ] Press Cmd+Return. Overlay dismisses.
- [ ] Paste. JSON has mode="frozen", coords correspond to the chosen UI element.
- [ ] `open` the clean PNG. The element you annotated is at the recorded coords.

**Discard flow:**
- [ ] Press hotkey, draw a rect, press Esc. Modal appears.
- [ ] Click "Discard." Overlay dismisses. Clipboard unchanged.
- [ ] Press hotkey, draw a rect, press Esc. Click "Cancel." Modal closes, rect still there.
- [ ] Press Esc again, click "Save." Overlay dismisses, clipboard has JSON.

**Stability:**
- [ ] Repeat hotkey + draw + save cycle 5+ times. No crashes, no leaked windows.

**Automated checks:**
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --lib` — all green.
- [ ] `npm test` — all green.
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --no-deps -- -D warnings` — exit 0.
- [ ] `cargo fmt --check --manifest-path src-tauri/Cargo.toml` — exit 0.
- [ ] `npx tsc --noEmit` — exit 0.

- [ ] **Step 4: On full pass, commit a marker:**

```bash
git commit --allow-empty -m "v0.2-drawing slice acceptance pass"
```

---

## After this plan

Subsequent plans (each on their own branch):

- **v0.2 follow-up (annotated PNG)** — annotated-PNG generation was pulled forward from v0.3 before merge. See [`2026-05-22-glitch-spot-v0.2-annotated-png.md`](./2026-05-22-glitch-spot-v0.2-annotated-png.md).
- **v0.3-persistence** — `~/.glitch-spot/snapshots/{id}/{clean.png, annotated.png, snapshot.json}`, 24h auto-cleanup. JSON `images.clean` switches from `/tmp` to the persistence dir. (Annotated-PNG generation is no longer in this slice — it ships in v0.2.)
- **v0.4-mcp** — local MCP server. Decides stdio child binary vs. axum HTTP. Implements `wait_for_snapshot`, `get_latest_snapshot`, `get_snapshot(id)` tools/resources. Ships Cursor and Claude Code config snippets.
- **v1.0-windows-and-relay** — Windows desktop port + `@glitch-spot/web` SDK + Cloudflare relay.

---

## Self-review against the spec

| Scope item | Where addressed |
|---|---|
| Drag-to-draw rectangles in both modes | M3 (reducer), M4 (mouse layer) — runs in any mode |
| Per-rectangle inline notes | M5 |
| Delete rectangle | M6 |
| Cmd+Z undo | M3.3 (reducer), M7 (binding) |
| Floating Save toolbar | M8 |
| Cmd+Return / Save button → snapshot | M10 + M11 |
| Esc-with-content discard modal | M9 |
| Esc-without-content immediate dismiss | M9 Step 2 |
| Snapshot JSON schema | M1 (types), M10 (assembly) |
| Clean PNG to `/tmp/glitch-spot-<id>.png` | M10 |
| Clipboard write | M10 (plugin + invoke) |
| Vitest setup | M2 |
| Reducer tests | M3 |

No spec items remain unaddressed in this plan. All `TODO`-like placeholders in the milestone steps either (a) point at later milestones explicitly, or (b) are deferred-by-design and called out in §"Explicitly out of scope (deferred)."
