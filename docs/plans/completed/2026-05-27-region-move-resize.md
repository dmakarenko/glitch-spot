# Region Move & Resize Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> Source seed: [`docs/seeds/2026-05-27-region-move-resize-seed.md`](../seeds/2026-05-27-region-move-resize-seed.md).

**Goal:** Allow the user to drag a rectangle by its body to move it, and drag any of its four corner handles to resize it, before saving the snapshot. Applies to every rectangle in the current overlay session; clamped to viewport with the existing `MIN_RECT_DIMENSION` floor; each gesture is one undo step.

**Architecture:** Generalise the reducer's single-purpose `draggingId` into a tagged `ActiveGesture` (`create | move | resize`). The reducer dispatches on `state.active.kind` inside `POINTER_TO` and `END_GESTURE`. Two new history entries (`move`, `resize`) hold the pre-gesture geometry for undo. Pure constraint helpers (`clampMoveDelta`, `clampResize`) live in a new `transform.ts`. `DrawingLayer.tsx` adds per-rect hover state, corner-handle children, and flips the rect wrapper's `pointerEvents` from `'none'` to `'auto'` so the body can catch `mousedown` for move.

**Tech Stack:** React 19 + TypeScript (Vite), Vitest. No IPC change, no Rust change.

---

## File structure

```
src/overlay/annotations/
  types.ts             # MODIFY — ActiveGesture, new actions, new history kinds, drop Rect.anchorX/Y
  state.ts             # MODIFY — rename actions, new active gesture, move/resize branches, new UNDO branches
  state.test.ts        # MODIFY — rename action names, add move/resize coverage
  transform.ts         # NEW    — clampMoveDelta + clampResize, both pure
  transform.test.ts    # NEW    — pure unit tests for the two helpers
  DrawingLayer.tsx     # MODIFY — hover state, corner handles, body mousedown for move, cursor styles
src/overlay/
  Overlay.tsx          # MODIFY — rename action types in keyboard handlers
```

`packages/electron/`, `src-tauri/`, `src/overlay/annotate.ts`, `src/overlay/Toolbar.tsx`, `src/overlay/annotations/RectangleNote.tsx`, `src/overlay/annotations/DiscardModal.tsx` — all untouched.

---

## Shared types (referenced across tasks)

```ts
// In src/overlay/annotations/types.ts after Task 2 lands.

export type Corner = 'nw' | 'ne' | 'sw' | 'se';

export type Rect = {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  note: string;
};

export type ActiveGesture =
  | { kind: 'create'; id: string; anchorX: number; anchorY: number }
  | { kind: 'move';   id: string; pointerStart: { x: number; y: number }; rectStart: { x: number; y: number } }
  | { kind: 'resize'; id: string; corner: Corner; anchor: { x: number; y: number } };

export type AnnotationsState = {
  rects: Rect[];
  history: HistoryEntry[];
  active: ActiveGesture | null;
  lastCreatedId: string | null;
};

export type HistoryEntry =
  | { kind: 'create';   id: string }
  | { kind: 'delete';   rect: Rect; index: number }
  | { kind: 'noteEdit'; id: string; previousNote: string }
  | { kind: 'move';     id: string; previousX: number; previousY: number }
  | { kind: 'resize';   id: string; previous: { x: number; y: number; w: number; h: number } };

export type AnnotationsAction =
  | { type: 'BEGIN_CREATE'; id: string; x: number; y: number }
  | { type: 'BEGIN_MOVE';   id: string; pointerX: number; pointerY: number }
  | { type: 'BEGIN_RESIZE'; id: string; corner: Corner; pointerX: number; pointerY: number }
  | { type: 'POINTER_TO';   x: number; y: number }
  | { type: 'END_GESTURE' }
  | { type: 'SET_NOTE';     id: string; note: string }
  | { type: 'DELETE_RECT';  id: string }
  | { type: 'UNDO' }
  | { type: 'CLEAR' };
```

The reducer signature, `MIN_RECT_DIMENSION`, and `initialState` shape stay otherwise the same.

The viewport size needed for clamping is read at dispatch time from `window.innerWidth` / `window.innerHeight` and passed *into* the action. Two action shapes are extended:

```ts
| { type: 'POINTER_TO'; x: number; y: number; viewportW: number; viewportH: number }
| { type: 'BEGIN_RESIZE'; id: string; corner: Corner; pointerX: number; pointerY: number; viewportW: number; viewportH: number }
| { type: 'BEGIN_MOVE';   id: string; pointerX: number; pointerY: number; viewportW: number; viewportH: number }
```

Adding the viewport to every gesture-bearing action keeps the reducer pure (no globals, no `window` reads from inside it). Use this exact extended shape everywhere.

---

## Task 1: Pure constraint helpers (`transform.ts`)

**Files:**
- Create: `src/overlay/annotations/transform.ts`
- Create: `src/overlay/annotations/transform.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/overlay/annotations/transform.test.ts` with this exact content:

```ts
// ABOUTME: Unit tests for the pure constraint helpers used by the reducer when
// ABOUTME: moving or resizing an annotation rectangle.
import { describe, it, expect } from 'vitest';
import { clampMoveDelta, clampResize } from './transform';
import { MIN_RECT_DIMENSION } from './state';

const VW = 1000;
const VH = 800;

describe('clampMoveDelta', () => {
  it('applies the full delta when the result stays inside the viewport', () => {
    const out = clampMoveDelta({ x: 100, y: 100, w: 50, h: 50 }, 20, -30, { w: VW, h: VH });
    expect(out).toEqual({ x: 120, y: 70 });
  });

  it('clamps x at 0 when the rect would cross the left edge', () => {
    const out = clampMoveDelta({ x: 10, y: 100, w: 50, h: 50 }, -50, 0, { w: VW, h: VH });
    expect(out).toEqual({ x: 0, y: 100 });
  });

  it('clamps y at 0 when the rect would cross the top edge', () => {
    const out = clampMoveDelta({ x: 100, y: 10, w: 50, h: 50 }, 0, -50, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 0 });
  });

  it('clamps x so the rect stays inside the right edge', () => {
    const out = clampMoveDelta({ x: 900, y: 100, w: 200, h: 50 }, 500, 0, { w: VW, h: VH });
    expect(out).toEqual({ x: VW - 200, y: 100 });
  });

  it('clamps y so the rect stays inside the bottom edge', () => {
    const out = clampMoveDelta({ x: 100, y: 700, w: 50, h: 200 }, 0, 500, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: VH - 200 });
  });
});

describe('clampResize', () => {
  // anchor = the corner that stays pinned; pointer = the corner being dragged.
  it('NW corner drag towards SE shrinks rect with SE anchor fixed', () => {
    const out = clampResize('nw', { x: 500, y: 400 }, { x: 200, y: 100 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 200, y: 100, w: 300, h: 300 });
  });

  it('SE corner drag towards NW shrinks rect with NW anchor fixed', () => {
    const out = clampResize('se', { x: 100, y: 100 }, { x: 400, y: 300 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 100, w: 300, h: 200 });
  });

  it('NE corner drag', () => {
    const out = clampResize('ne', { x: 100, y: 400 }, { x: 500, y: 100 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 100, w: 400, h: 300 });
  });

  it('SW corner drag', () => {
    const out = clampResize('sw', { x: 500, y: 100 }, { x: 100, y: 400 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 100, w: 400, h: 300 });
  });

  it('stops at MIN_RECT_DIMENSION when the dragged corner approaches the anchor', () => {
    const out = clampResize('se', { x: 100, y: 100 }, { x: 105, y: 110 }, { w: VW, h: VH });
    expect(out).toEqual({
      x: 100,
      y: 100,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it('refuses to flip past the anchor (SE corner dragged past NW)', () => {
    const out = clampResize('se', { x: 100, y: 100 }, { x: 50, y: 50 }, { w: VW, h: VH });
    expect(out).toEqual({
      x: 100,
      y: 100,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it('refuses to flip past the anchor (NW corner dragged past SE)', () => {
    const out = clampResize('nw', { x: 100, y: 100 }, { x: 200, y: 200 }, { w: VW, h: VH });
    expect(out).toEqual({
      x: 100 - MIN_RECT_DIMENSION,
      y: 100 - MIN_RECT_DIMENSION,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it('clamps the dragged corner inside the viewport (SE corner beyond right edge)', () => {
    const out = clampResize('se', { x: 100, y: 100 }, { x: VW + 200, y: 300 }, { w: VW, h: VH });
    expect(out.x + out.w).toBe(VW);
  });

  it('clamps the dragged corner inside the viewport (NW corner beyond top edge)', () => {
    const out = clampResize('nw', { x: 500, y: 400 }, { x: 200, y: -50 }, { w: VW, h: VH });
    expect(out.y).toBe(0);
    expect(out.y + out.h).toBe(400);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run from the worktree root: `npm test -- src/overlay/annotations/transform.test.ts`
Expected: FAIL with module-not-found for `./transform`.

- [ ] **Step 3: Write the implementation**

Create `src/overlay/annotations/transform.ts` with this exact content:

```ts
// ABOUTME: Pure constraint helpers for moving and resizing annotation rectangles.
// ABOUTME: Clamps to viewport bounds and enforces the MIN_RECT_DIMENSION floor.

import { MIN_RECT_DIMENSION } from './state';
import type { Corner } from './types';

export type Viewport = { w: number; h: number };
export type Point = { x: number; y: number };

export function clampMoveDelta(
  rect: { x: number; y: number; w: number; h: number },
  deltaX: number,
  deltaY: number,
  viewport: Viewport,
): Point {
  const x = clamp(rect.x + deltaX, 0, viewport.w - rect.w);
  const y = clamp(rect.y + deltaY, 0, viewport.h - rect.h);
  return { x, y };
}

export function clampResize(
  corner: Corner,
  anchor: Point,
  pointer: Point,
  viewport: Viewport,
): { x: number; y: number; w: number; h: number } {
  const pointerX = clamp(pointer.x, 0, viewport.w);
  const pointerY = clamp(pointer.y, 0, viewport.h);

  let x: number;
  let y: number;
  let w: number;
  let h: number;

  switch (corner) {
    case 'nw':
      // Anchor is SE corner; pointer is NW corner.
      x = Math.min(pointerX, anchor.x - MIN_RECT_DIMENSION);
      y = Math.min(pointerY, anchor.y - MIN_RECT_DIMENSION);
      w = anchor.x - x;
      h = anchor.y - y;
      break;
    case 'ne':
      // Anchor is SW corner; pointer is NE corner.
      x = anchor.x;
      y = Math.min(pointerY, anchor.y - MIN_RECT_DIMENSION);
      w = Math.max(pointerX - anchor.x, MIN_RECT_DIMENSION);
      h = anchor.y - y;
      break;
    case 'sw':
      // Anchor is NE corner; pointer is SW corner.
      x = Math.min(pointerX, anchor.x - MIN_RECT_DIMENSION);
      y = anchor.y;
      w = anchor.x - x;
      h = Math.max(pointerY - anchor.y, MIN_RECT_DIMENSION);
      break;
    case 'se':
      // Anchor is NW corner; pointer is SE corner.
      x = anchor.x;
      y = anchor.y;
      w = Math.max(pointerX - anchor.x, MIN_RECT_DIMENSION);
      h = Math.max(pointerY - anchor.y, MIN_RECT_DIMENSION);
      break;
  }

  return { x, y, w, h };
}

function clamp(v: number, lo: number, hi: number): number {
  if (hi < lo) return lo;
  return Math.min(hi, Math.max(lo, v));
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm test -- src/overlay/annotations/transform.test.ts`
Expected: PASS, all 12 tests.

- [ ] **Step 5: Commit**

```bash
git add src/overlay/annotations/transform.ts src/overlay/annotations/transform.test.ts
git commit -m "feat(overlay): add clampMoveDelta and clampResize helpers"
```

---

## Task 2: Rename action types + introduce `ActiveGesture` shape

**Files:**
- Modify: `src/overlay/annotations/types.ts`
- Modify: `src/overlay/annotations/state.ts`
- Modify: `src/overlay/annotations/state.test.ts`
- Modify: `src/overlay/Overlay.tsx`
- Modify: `src/overlay/annotations/DrawingLayer.tsx` (callers that reference the old action names — only the rect-creation flow)

This task is purely mechanical — no new behavior. Rename:
- `BEGIN_DRAG` → `BEGIN_CREATE`
- `DRAG_TO` → `POINTER_TO`
- `END_DRAG` → `END_GESTURE`

Migrate `state.draggingId: string | null` to `state.active: ActiveGesture | null` with the `create` variant only. Move the anchor data from `Rect.anchorX/anchorY` onto `ActiveGesture`. After this task, the reducer behavior is byte-identical to today; the state shape and action names are different.

- [ ] **Step 1: Replace `types.ts`**

Open `src/overlay/annotations/types.ts`. Replace the entire file contents with:

```ts
// ABOUTME: Shared annotation types for the overlay's drawing layer + Rust IPC payload.

export type Corner = 'nw' | 'ne' | 'sw' | 'se';

export type Rect = {
  id: string;
  // CSS-pixel coordinates inside the overlay's content area.
  x: number;
  y: number;
  w: number;
  h: number;
  note: string;
};

export type ActiveGesture =
  | { kind: 'create'; id: string; anchorX: number; anchorY: number }
  | { kind: 'move'; id: string; pointerStart: { x: number; y: number }; rectStart: { x: number; y: number } }
  | { kind: 'resize'; id: string; corner: Corner; anchor: { x: number; y: number } };

export type AnnotationsState = {
  rects: Rect[];
  history: HistoryEntry[];
  active: ActiveGesture | null;
  lastCreatedId: string | null;
};

export type HistoryEntry =
  | { kind: 'create'; id: string }
  | { kind: 'delete'; rect: Rect; index: number }
  | { kind: 'noteEdit'; id: string; previousNote: string }
  | { kind: 'move'; id: string; previousX: number; previousY: number }
  | { kind: 'resize'; id: string; previous: { x: number; y: number; w: number; h: number } };

export type AnnotationsAction =
  | { type: 'BEGIN_CREATE'; id: string; x: number; y: number }
  | {
      type: 'BEGIN_MOVE';
      id: string;
      pointerX: number;
      pointerY: number;
      viewportW: number;
      viewportH: number;
    }
  | {
      type: 'BEGIN_RESIZE';
      id: string;
      corner: Corner;
      pointerX: number;
      pointerY: number;
      viewportW: number;
      viewportH: number;
    }
  | {
      type: 'POINTER_TO';
      x: number;
      y: number;
      viewportW: number;
      viewportH: number;
    }
  | { type: 'END_GESTURE' }
  | { type: 'SET_NOTE'; id: string; note: string }
  | { type: 'DELETE_RECT'; id: string }
  | { type: 'UNDO' }
  | { type: 'CLEAR' };
```

`label` is already present on `Rect` per the region-labels feature that just shipped; keep it. The `anchorX/anchorY` fields are gone — they live on `ActiveGesture` now.

- [ ] **Step 2: Replace `state.ts`**

Open `src/overlay/annotations/state.ts`. Replace the entire file contents with:

```ts
// ABOUTME: useReducer state for the overlay's annotation layer. No React deps here —
// ABOUTME: keep it pure so the logic stays testable in plain Node/vitest.

import type {
  ActiveGesture,
  AnnotationsAction,
  AnnotationsState,
  HistoryEntry,
  Rect,
} from "./types";

// Smallest committed-rectangle dimension. Aligned with DrawingLayer's delete-X
// visibility threshold so every saved rectangle has a working delete button,
// and a stray click without a drag is discarded as accidental.
export const MIN_RECT_DIMENSION = 25;

export const initialState: AnnotationsState = {
  rects: [],
  history: [],
  active: null,
  lastCreatedId: null,
};

export function reducer(
  state: AnnotationsState,
  action: AnnotationsAction,
): AnnotationsState {
  switch (action.type) {
    case "BEGIN_CREATE": {
      const rect: Rect = {
        id: action.id,
        x: action.x,
        y: action.y,
        w: 0,
        h: 0,
        note: "",
      };
      const active: ActiveGesture = {
        kind: "create",
        id: action.id,
        anchorX: action.x,
        anchorY: action.y,
      };
      const history: HistoryEntry[] = [
        ...state.history,
        { kind: "create", id: action.id },
      ];
      return {
        ...state,
        rects: [...state.rects, rect],
        history,
        active,
        lastCreatedId: action.id,
      };
    }

    case "POINTER_TO": {
      if (!state.active) return state;
      if (state.active.kind === "create") {
        const { id, anchorX, anchorY } = state.active;
        const rects = state.rects.map((r) =>
          r.id === id ? extendTo(r, action.x, action.y, anchorX, anchorY) : r,
        );
        return { ...state, rects };
      }
      return state;
    }

    case "END_GESTURE": {
      if (!state.active) return state;
      if (state.active.kind === "create") {
        const id = state.active.id;
        const drawn = state.rects.find((r) => r.id === id);
        if (!drawn) {
          return { ...state, active: null };
        }
        const tooSmall =
          drawn.w < MIN_RECT_DIMENSION || drawn.h < MIN_RECT_DIMENSION;
        if (tooSmall) {
          return {
            ...state,
            rects: state.rects.filter((r) => r.id !== id),
            history: state.history.filter(
              (h) => !(h.kind === "create" && h.id === id),
            ),
            active: null,
            lastCreatedId:
              state.lastCreatedId === id ? null : state.lastCreatedId,
          };
        }
        return { ...state, active: null };
      }
      return { ...state, active: null };
    }

    case "SET_NOTE": {
      const rect = state.rects.find((r) => r.id === action.id);
      if (!rect) return state;
      const rects = state.rects.map((r) =>
        r.id === action.id ? { ...r, note: action.note } : r,
      );
      const alreadyEditing = state.history.some(
        (h) => h.kind === "noteEdit" && h.id === action.id,
      );
      const history: HistoryEntry[] = alreadyEditing
        ? state.history
        : [
            ...state.history,
            { kind: "noteEdit", id: action.id, previousNote: rect.note },
          ];
      return { ...state, rects, history };
    }

    case "DELETE_RECT": {
      const index = state.rects.findIndex((r) => r.id === action.id);
      if (index < 0) return state;
      const rect = state.rects[index];
      const rects = state.rects.filter((r) => r.id !== action.id);
      const history: HistoryEntry[] = [
        ...state.history,
        { kind: "delete", rect, index },
      ];
      return { ...state, rects, history };
    }

    case "CLEAR":
      return initialState;

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
            lastCreatedId:
              last.id === state.lastCreatedId ? null : state.lastCreatedId,
          };
        case "delete": {
          const rects = [...state.rects];
          const insertAt = Math.min(last.index, rects.length);
          rects.splice(insertAt, 0, last.rect);
          return { ...state, rects, history };
        }
        case "noteEdit": {
          const rects = state.rects.map((r) =>
            r.id === last.id ? { ...r, note: last.previousNote } : r,
          );
          return { ...state, rects, history };
        }
        // The "move" and "resize" undo branches are added in later tasks.
      }
      return state;
    }

    default:
      return state;
  }
}

function extendTo(
  rect: Rect,
  cursorX: number,
  cursorY: number,
  anchorX: number,
  anchorY: number,
): Rect {
  const x = Math.min(anchorX, cursorX);
  const y = Math.min(anchorY, cursorY);
  const w = Math.abs(cursorX - anchorX);
  const h = Math.abs(cursorY - anchorY);
  return { ...rect, x, y, w, h };
}
```

The `BEGIN_MOVE`, `BEGIN_RESIZE`, and the move/resize branches inside `POINTER_TO` and `END_GESTURE` are no-ops in this task (the switch falls through). They get filled in by Tasks 3, 4, 5. The `UNDO` switch has a `// The "move" and "resize" undo branches are added in later tasks.` comment marking where those land.

Note: `Rect` here keeps the same fields as on `main` — `{ id, x, y, w, h, note }`. The seed mentioned a `label` field on `Rect` but that's incorrect: labels are derived from array index at render time (live overlay's `formatLabel(idx)`) and at snapshot-build time (Electron `main.ts`, Rust `build.rs`). Nothing stores `label` on the React-side `Rect`. The `anchorX/anchorY` optional fields used today for the creation drag move onto `ActiveGesture` — that part of the seed is correct.

- [ ] **Step 3: Replace `state.test.ts`**

Open `src/overlay/annotations/state.test.ts`. Replace the entire file contents with:

```ts
// ABOUTME: Reducer tests for the overlay's annotation state. Pure data, no DOM.
import { describe, expect, it } from "vitest";
import { reducer, initialState, MIN_RECT_DIMENSION } from "./state";

const VW = 1000;
const VH = 800;

function pointerTo(x: number, y: number) {
  return { type: "POINTER_TO" as const, x, y, viewportW: VW, viewportH: VH };
}

describe("annotations reducer", () => {
  it("starts with no rects, no history, no active gesture", () => {
    expect(initialState).toEqual({
      rects: [],
      history: [],
      active: null,
      lastCreatedId: null,
    });
  });

  it("BEGIN_CREATE creates a zero-size rect anchored at the start point", () => {
    const s = reducer(initialState, {
      type: "BEGIN_CREATE",
      id: "r1",
      x: 100,
      y: 50,
    });
    expect(s.rects).toHaveLength(1);
    expect(s.rects[0]).toMatchObject({
      id: "r1",
      x: 100,
      y: 50,
      w: 0,
      h: 0,
      note: "",
    });
    expect(s.active).toEqual({
      kind: "create",
      id: "r1",
      anchorX: 100,
      anchorY: 50,
    });
    expect(s.history).toEqual([{ kind: "create", id: "r1" }]);
  });

  it("POINTER_TO updates the dragging rect's bottom-right while create is active", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 50 });
    s = reducer(s, pointerTo(200, 100));
    expect(s.rects[0]).toMatchObject({ x: 100, y: 50, w: 100, h: 50 });
  });

  it("POINTER_TO handles drags up and to the left by flipping origin", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 200, y: 100 });
    s = reducer(s, pointerTo(100, 50));
    expect(s.rects[0]).toMatchObject({ x: 100, y: 50, w: 100, h: 50 });
  });

  it("END_GESTURE clears active without removing the rect", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 50 });
    s = reducer(s, pointerTo(200, 100));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.active).toBeNull();
    expect(s.rects).toHaveLength(1);
  });

  it("POINTER_TO is a no-op when there is no active gesture", () => {
    const s = reducer(initialState, pointerTo(10, 10));
    expect(s).toEqual(initialState);
  });

  it("BEGIN_CREATE sets lastCreatedId so the new rect's note can auto-focus", () => {
    const s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    expect(s.lastCreatedId).toBe("r1");
  });
});

describe("notes and deletion", () => {
  function drawn(id: string): import("./types").AnnotationsState {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id, x: 0, y: 0 });
    s = reducer(s, pointerTo(100, 50));
    return reducer(s, { type: "END_GESTURE" });
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
    s = reducer(s, { type: "BEGIN_CREATE", id: "r2", x: 200, y: 0 });
    s = reducer(s, pointerTo(300, 80));
    s = reducer(s, { type: "END_GESTURE" });
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

describe("undo", () => {
  function drawn(id: string, state = initialState): import("./types").AnnotationsState {
    let s = reducer(state, { type: "BEGIN_CREATE", id, x: 0, y: 0 });
    s = reducer(s, pointerTo(100, 50));
    return reducer(s, { type: "END_GESTURE" });
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

  it("UNDO of a create clears lastCreatedId when undoing the most recent create", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    s = reducer(s, pointerTo(100, 50));
    s = reducer(s, { type: "END_GESTURE" });
    s = reducer(s, { type: "UNDO" });
    expect(s.lastCreatedId).toBeNull();
  });
});

describe("min-size guard on END_GESTURE", () => {
  it("MIN_RECT_DIMENSION matches the X-button visibility threshold (25)", () => {
    expect(MIN_RECT_DIMENSION).toBe(25);
  });

  it("END_GESTURE after a pure click (no drag) discards the zero-size rect entirely", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 50 });
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects).toEqual([]);
    expect(s.history).toEqual([]);
    expect(s.active).toBeNull();
    expect(s.lastCreatedId).toBeNull();
  });

  it("END_GESTURE discards a sub-MIN_RECT_DIMENSION drag (10x10)", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 50 });
    s = reducer(s, pointerTo(110, 60));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects).toEqual([]);
    expect(s.history).toEqual([]);
  });

  it("END_GESTURE keeps a rect that meets the threshold (25x25)", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    s = reducer(s, pointerTo(25, 25));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects).toHaveLength(1);
    expect(s.rects[0]).toMatchObject({ w: 25, h: 25 });
  });

  it("END_GESTURE discards a wide-but-flat rect (60x10) because height is below the threshold", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    s = reducer(s, pointerTo(60, 10));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects).toEqual([]);
  });

  it("discarding a too-small rect leaves any previously committed rectangles untouched", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    s = reducer(s, pointerTo(100, 80));
    s = reducer(s, { type: "END_GESTURE" });
    s = reducer(s, { type: "BEGIN_CREATE", id: "r2", x: 200, y: 200 });
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects.map((r) => r.id)).toEqual(["r1"]);
    expect(s.history).toEqual([{ kind: "create", id: "r1" }]);
  });
});
```

- [ ] **Step 4: Update `Overlay.tsx`**

Open `src/overlay/Overlay.tsx`. Locate the `handleSave` and `useEffect` blocks that reference the old action types. They don't dispatch the renamed actions directly — only `UNDO` and `CLEAR` are used there, both unchanged. So no edit is needed in `Overlay.tsx` for this task.

To confirm, run from the worktree root:

```bash
grep -n "BEGIN_DRAG\|DRAG_TO\|END_DRAG\|draggingId" src/overlay/Overlay.tsx
```

Expected: no matches. If matches appear, replace them in `Overlay.tsx` with the new names (`BEGIN_CREATE`, `POINTER_TO`, `END_GESTURE`, `state.active != null` as appropriate) — but the current file should not reference these.

- [ ] **Step 5: Update `DrawingLayer.tsx` action call sites**

Open `src/overlay/annotations/DrawingLayer.tsx`. Find these three dispatch sites (lines around 50, 62, 70) and rename:

```tsx
// Before:
dispatch({ type: "BEGIN_DRAG", id: nextRectId(), x, y });
// After:
dispatch({ type: "BEGIN_CREATE", id: nextRectId(), x, y });
```

```tsx
// Before:
dispatch({ type: "DRAG_TO", x, y });
// After:
dispatch({
  type: "POINTER_TO",
  x,
  y,
  viewportW: window.innerWidth,
  viewportH: window.innerHeight,
});
```

```tsx
// Before:
dispatch({ type: "END_DRAG" });
// After:
dispatch({ type: "END_GESTURE" });
```

Also rename the `state.draggingId` read at the top of `onMouseMove` and `onMouseUp` callbacks:

```tsx
// Before:
if (!state.draggingId) return;
// After:
if (!state.active) return;
```

Both callbacks. The same `state.active != null` check is what both old reads were really expressing.

- [ ] **Step 6: Run the full test suite to verify everything is green**

Run: `npm test`
Expected: PASS — all suites still green (the renamed action names now match the renamed reducer; existing behavior is unchanged).

- [ ] **Step 7: Commit**

```bash
git add src/overlay/annotations/types.ts src/overlay/annotations/state.ts src/overlay/annotations/state.test.ts src/overlay/annotations/DrawingLayer.tsx
git commit -m "refactor(overlay): rename create-gesture actions and introduce ActiveGesture"
```

If a `git status` after this commit still reports modifications to `Overlay.tsx` from Step 4, include it in the commit too.

---

## Task 3: Implement `BEGIN_MOVE` + move branch of `POINTER_TO` / `END_GESTURE`

**Files:**
- Modify: `src/overlay/annotations/state.ts`
- Modify: `src/overlay/annotations/state.test.ts`

This task adds the move gesture end-to-end in the reducer. No UI changes yet.

- [ ] **Step 1: Write the failing tests**

Open `src/overlay/annotations/state.test.ts`. Append this `describe` block at the end of the file:

```ts
describe("move gesture", () => {
  function drawn(id: string, x: number, y: number, w: number, h: number): import("./types").AnnotationsState {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id, x, y });
    s = reducer(s, pointerTo(x + w, y + h));
    return reducer(s, { type: "END_GESTURE" });
  }

  it("BEGIN_MOVE sets active=move and records the previous position in history", () => {
    const start = drawn("r1", 100, 100, 50, 50);
    const s = reducer(start, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 130,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.active).toEqual({
      kind: "move",
      id: "r1",
      pointerStart: { x: 120, y: 130 },
      rectStart: { x: 100, y: 100 },
    });
    const last = s.history[s.history.length - 1];
    expect(last).toEqual({ kind: "move", id: "r1", previousX: 100, previousY: 100 });
  });

  it("POINTER_TO during move translates the rect by the pointer delta", () => {
    let s = drawn("r1", 100, 100, 50, 50);
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 130,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(200, 200));
    expect(s.rects[0]).toMatchObject({ x: 180, y: 170, w: 50, h: 50 });
  });

  it("POINTER_TO during move clamps to viewport edges", () => {
    let s = drawn("r1", 100, 100, 50, 50);
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 110,
      pointerY: 110,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(-500, -500));
    expect(s.rects[0]).toMatchObject({ x: 0, y: 0, w: 50, h: 50 });
  });

  it("END_GESTURE after a move clears active and keeps the move history entry", () => {
    let s = drawn("r1", 100, 100, 50, 50);
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 130,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(200, 200));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.active).toBeNull();
    expect(s.rects[0]).toMatchObject({ x: 180, y: 170 });
    const last = s.history[s.history.length - 1];
    expect(last).toEqual({ kind: "move", id: "r1", previousX: 100, previousY: 100 });
  });

  it("END_GESTURE after BEGIN_MOVE with no POINTER_TO pops the no-op history entry", () => {
    let s = drawn("r1", 100, 100, 50, 50);
    const before = s.history.length;
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 130,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.history.length).toBe(before + 1);
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.history.length).toBe(before);
    expect(s.history[s.history.length - 1]).not.toMatchObject({ kind: "move" });
  });

  it("BEGIN_MOVE on a non-existent id is a no-op", () => {
    const start = drawn("r1", 100, 100, 50, 50);
    const s = reducer(start, {
      type: "BEGIN_MOVE",
      id: "ghost",
      pointerX: 0,
      pointerY: 0,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s).toEqual(start);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- src/overlay/annotations/state.test.ts`
Expected: FAIL on the new `move gesture` block — `BEGIN_MOVE` currently isn't handled (falls through the default branch).

- [ ] **Step 3: Implement the move branch**

Open `src/overlay/annotations/state.ts`. Add a top-of-file import:

```ts
import { clampMoveDelta } from "./transform";
```

Add a new case to the reducer's switch, between `BEGIN_CREATE` and `POINTER_TO`:

```ts
    case "BEGIN_MOVE": {
      const rect = state.rects.find((r) => r.id === action.id);
      if (!rect) return state;
      const active: ActiveGesture = {
        kind: "move",
        id: action.id,
        pointerStart: { x: action.pointerX, y: action.pointerY },
        rectStart: { x: rect.x, y: rect.y },
      };
      const history: HistoryEntry[] = [
        ...state.history,
        { kind: "move", id: action.id, previousX: rect.x, previousY: rect.y },
      ];
      return { ...state, active, history };
    }
```

Extend the existing `POINTER_TO` case so its inner switch also handles `move`:

```ts
    case "POINTER_TO": {
      if (!state.active) return state;
      if (state.active.kind === "create") {
        const { id, anchorX, anchorY } = state.active;
        const rects = state.rects.map((r) =>
          r.id === id ? extendTo(r, action.x, action.y, anchorX, anchorY) : r,
        );
        return { ...state, rects };
      }
      if (state.active.kind === "move") {
        const { id, pointerStart, rectStart } = state.active;
        const rect = state.rects.find((r) => r.id === id);
        if (!rect) return state;
        const deltaX = action.x - pointerStart.x;
        const deltaY = action.y - pointerStart.y;
        const startRect = { ...rect, x: rectStart.x, y: rectStart.y };
        const { x, y } = clampMoveDelta(
          startRect,
          deltaX,
          deltaY,
          { w: action.viewportW, h: action.viewportH },
        );
        const rects = state.rects.map((r) => (r.id === id ? { ...r, x, y } : r));
        return { ...state, rects };
      }
      return state;
    }
```

Extend the existing `END_GESTURE` case so move gets the no-op-pop behavior:

```ts
    case "END_GESTURE": {
      if (!state.active) return state;
      if (state.active.kind === "create") {
        // (unchanged from Task 2 — keep the existing block)
        // ...
      }
      if (state.active.kind === "move") {
        const { id } = state.active;
        const rect = state.rects.find((r) => r.id === id);
        const last = state.history[state.history.length - 1];
        if (
          rect &&
          last &&
          last.kind === "move" &&
          last.id === id &&
          last.previousX === rect.x &&
          last.previousY === rect.y
        ) {
          // No-op move (mousedown without movement). Pop the history entry.
          const history = state.history.slice(0, -1);
          return { ...state, active: null, history };
        }
        return { ...state, active: null };
      }
      return { ...state, active: null };
    }
```

Keep the existing "create" branch inside `END_GESTURE` exactly as it was after Task 2 — just nest the new `move` branch beside it.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm test -- src/overlay/annotations/state.test.ts`
Expected: PASS — the new `move gesture` describe block plus all existing tests.

- [ ] **Step 5: Run the full test suite**

Run: `npm test`
Expected: PASS — all suites green.

- [ ] **Step 6: Commit**

```bash
git add src/overlay/annotations/state.ts src/overlay/annotations/state.test.ts
git commit -m "feat(overlay): support moving a rectangle by dragging its body"
```

---

## Task 4: Implement `BEGIN_RESIZE` + resize branch of `POINTER_TO` / `END_GESTURE`

**Files:**
- Modify: `src/overlay/annotations/state.ts`
- Modify: `src/overlay/annotations/state.test.ts`

- [ ] **Step 1: Write the failing tests**

Open `src/overlay/annotations/state.test.ts`. Append this `describe` block at the end of the file:

```ts
describe("resize gesture", () => {
  function drawn(id: string, x: number, y: number, w: number, h: number): import("./types").AnnotationsState {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id, x, y });
    s = reducer(s, pointerTo(x + w, y + h));
    return reducer(s, { type: "END_GESTURE" });
  }

  it("BEGIN_RESIZE from SE corner anchors at NW and records previous geometry in history", () => {
    const start = drawn("r1", 100, 100, 200, 150);
    const s = reducer(start, {
      type: "BEGIN_RESIZE",
      id: "r1",
      corner: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.active).toEqual({
      kind: "resize",
      id: "r1",
      corner: "se",
      anchor: { x: 100, y: 100 },
    });
    const last = s.history[s.history.length - 1];
    expect(last).toEqual({
      kind: "resize",
      id: "r1",
      previous: { x: 100, y: 100, w: 200, h: 150 },
    });
  });

  it("BEGIN_RESIZE from NW corner anchors at SE", () => {
    const start = drawn("r1", 100, 100, 200, 150);
    const s = reducer(start, {
      type: "BEGIN_RESIZE",
      id: "r1",
      corner: "nw",
      pointerX: 100,
      pointerY: 100,
      viewportW: VW,
      viewportH: VH,
    });
    if (s.active && s.active.kind === "resize") {
      expect(s.active.anchor).toEqual({ x: 300, y: 250 });
    } else {
      throw new Error("expected active resize gesture");
    }
  });

  it("POINTER_TO during SE resize extends width/height with NW pinned", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      corner: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(400, 300));
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 300, h: 200 });
  });

  it("POINTER_TO during NW resize moves the top-left with SE pinned", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      corner: "nw",
      pointerX: 100,
      pointerY: 100,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(50, 50));
    expect(s.rects[0]).toMatchObject({ x: 50, y: 50, w: 250, h: 200 });
  });

  it("POINTER_TO past the anchor stops at MIN_RECT_DIMENSION (no flip)", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      corner: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(50, 50));
    expect(s.rects[0]).toMatchObject({
      x: 100,
      y: 100,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it("POINTER_TO during resize clamps the dragged corner to viewport", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      corner: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(VW + 500, VH + 500));
    const r = s.rects[0];
    expect(r.x + r.w).toBe(VW);
    expect(r.y + r.h).toBe(VH);
  });

  it("END_GESTURE after BEGIN_RESIZE with no POINTER_TO pops the no-op history entry", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    const before = s.history.length;
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      corner: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.history.length).toBe(before + 1);
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.history.length).toBe(before);
  });

  it("BEGIN_RESIZE on a non-existent id is a no-op", () => {
    const start = drawn("r1", 100, 100, 200, 150);
    const s = reducer(start, {
      type: "BEGIN_RESIZE",
      id: "ghost",
      corner: "se",
      pointerX: 0,
      pointerY: 0,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s).toEqual(start);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- src/overlay/annotations/state.test.ts`
Expected: FAIL on the new `resize gesture` block.

- [ ] **Step 3: Implement the resize branch**

Open `src/overlay/annotations/state.ts`. Extend the import at the top of the file:

```ts
import { clampMoveDelta, clampResize } from "./transform";
```

Add a new case to the reducer's switch, between `BEGIN_MOVE` and `POINTER_TO`:

```ts
    case "BEGIN_RESIZE": {
      const rect = state.rects.find((r) => r.id === action.id);
      if (!rect) return state;
      const anchor = anchorForCorner(rect, action.corner);
      const active: ActiveGesture = {
        kind: "resize",
        id: action.id,
        corner: action.corner,
        anchor,
      };
      const history: HistoryEntry[] = [
        ...state.history,
        {
          kind: "resize",
          id: action.id,
          previous: { x: rect.x, y: rect.y, w: rect.w, h: rect.h },
        },
      ];
      return { ...state, active, history };
    }
```

Extend the `POINTER_TO` case so it also handles `resize`:

```ts
      if (state.active.kind === "resize") {
        const { id, corner, anchor } = state.active;
        const geom = clampResize(
          corner,
          anchor,
          { x: action.x, y: action.y },
          { w: action.viewportW, h: action.viewportH },
        );
        const rects = state.rects.map((r) => (r.id === id ? { ...r, ...geom } : r));
        return { ...state, rects };
      }
```

Insert this block inside `POINTER_TO`, after the `move` branch and before the trailing `return state;`.

Extend `END_GESTURE` so resize gets the no-op-pop behavior alongside move:

```ts
      if (state.active.kind === "resize") {
        const { id } = state.active;
        const rect = state.rects.find((r) => r.id === id);
        const last = state.history[state.history.length - 1];
        if (
          rect &&
          last &&
          last.kind === "resize" &&
          last.id === id &&
          last.previous.x === rect.x &&
          last.previous.y === rect.y &&
          last.previous.w === rect.w &&
          last.previous.h === rect.h
        ) {
          const history = state.history.slice(0, -1);
          return { ...state, active: null, history };
        }
        return { ...state, active: null };
      }
```

Insert this block inside `END_GESTURE`, after the `move` branch and before the trailing `return { ...state, active: null };`.

Add the `anchorForCorner` helper at the bottom of the file (alongside `extendTo`):

```ts
function anchorForCorner(
  rect: { x: number; y: number; w: number; h: number },
  corner: import("./types").Corner,
): { x: number; y: number } {
  switch (corner) {
    case "nw":
      return { x: rect.x + rect.w, y: rect.y + rect.h };
    case "ne":
      return { x: rect.x, y: rect.y + rect.h };
    case "sw":
      return { x: rect.x + rect.w, y: rect.y };
    case "se":
      return { x: rect.x, y: rect.y };
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm test -- src/overlay/annotations/state.test.ts`
Expected: PASS — the new `resize gesture` describe block plus everything else.

- [ ] **Step 5: Run the full test suite**

Run: `npm test`
Expected: PASS — all suites green.

- [ ] **Step 6: Commit**

```bash
git add src/overlay/annotations/state.ts src/overlay/annotations/state.test.ts
git commit -m "feat(overlay): support resizing a rectangle from any of four corners"
```

---

## Task 5: Implement `UNDO` for move and resize history entries

**Files:**
- Modify: `src/overlay/annotations/state.ts`
- Modify: `src/overlay/annotations/state.test.ts`

- [ ] **Step 1: Write the failing tests**

Open `src/overlay/annotations/state.test.ts`. Inside the existing `describe("undo", ...)` block (NOT a new block — append to the existing one), add these tests at the end:

```ts
  it("UNDO of a move restores the previous position", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 100 });
    s = reducer(s, pointerTo(150, 150));
    s = reducer(s, { type: "END_GESTURE" });
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 120,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(220, 220));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects[0]).toMatchObject({ x: 200, y: 200 });
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 50, h: 50 });
  });

  it("UNDO of a resize restores the full previous geometry", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 100 });
    s = reducer(s, pointerTo(300, 250));
    s = reducer(s, { type: "END_GESTURE" });
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      corner: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(400, 300));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 300, h: 200 });
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 200, h: 150 });
  });

  it("UNDO walks back through move → resize in reverse order", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 100 });
    s = reducer(s, pointerTo(200, 200));
    s = reducer(s, { type: "END_GESTURE" });
    // first move (100,100) -> (150,150)
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 120,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(170, 170));
    s = reducer(s, { type: "END_GESTURE" });
    // then resize from (150,150,100,100) to wider
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      corner: "se",
      pointerX: 250,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(300, 300));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects[0]).toMatchObject({ x: 150, y: 150, w: 150, h: 150 });
    // Undo the resize first.
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0]).toMatchObject({ x: 150, y: 150, w: 100, h: 100 });
    // Then undo the move.
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 100, h: 100 });
  });
});
```

Make sure the closing `});` of the `describe("undo", ...)` block stays at the end.

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- src/overlay/annotations/state.test.ts`
Expected: FAIL — `UNDO` currently does nothing for `move` or `resize` history kinds (the inner switch has no cases for them, falling through to `return state`).

- [ ] **Step 3: Add the UNDO branches**

Open `src/overlay/annotations/state.ts`. Locate the `case "UNDO":` block. Inside its inner `switch (last.kind)`, just before the closing brace of that inner switch, add these two cases:

```ts
        case "move": {
          const rects = state.rects.map((r) =>
            r.id === last.id ? { ...r, x: last.previousX, y: last.previousY } : r,
          );
          return { ...state, rects, history };
        }
        case "resize": {
          const rects = state.rects.map((r) =>
            r.id === last.id ? { ...r, ...last.previous } : r,
          );
          return { ...state, rects, history };
        }
```

Remove the placeholder comment `// The "move" and "resize" undo branches are added in later tasks.` that Task 2 left there.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm test -- src/overlay/annotations/state.test.ts`
Expected: PASS.

- [ ] **Step 5: Run the full test suite**

Run: `npm test`
Expected: PASS — all suites green.

- [ ] **Step 6: Commit**

```bash
git add src/overlay/annotations/state.ts src/overlay/annotations/state.test.ts
git commit -m "feat(overlay): undo restores rect geometry after move and resize"
```

---

## Task 6: Wire `DrawingLayer.tsx` — body intercepts mousedown, hover-revealed corner handles

**Files:**
- Modify: `src/overlay/annotations/DrawingLayer.tsx`

No automated test for this task. The reducer is covered. The DOM wiring is verified manually via the Tauri dev app.

All pointer coordinates dispatched to the reducer are **surface-local** — relative to the drawing-surface element, not the viewport. The existing `BEGIN_CREATE` flow already does this conversion via `getBoundingClientRect`. The new `BEGIN_MOVE`, `BEGIN_RESIZE`, and the new `POINTER_TO` extension keep the same convention so the rect's stored `x/y` and the dispatched pointer coords always share a coordinate system.

Define a small helper at the top of the component to convert a React mouse event to surface-local coords using the existing `ref`. Pointer-state actions then read viewport dimensions from `window.innerWidth/innerHeight` and pass them through for the reducer's clamp helpers (viewport size is in surface-local terms because the surface is the same size as the viewport in this app — see `Overlay.tsx`'s `inset: 0` outer container).

- [ ] **Step 1: Add hover state and rewire the rect wrapper for move + resize**

Open `src/overlay/annotations/DrawingLayer.tsx`. At the top, alongside the existing imports, add:

```ts
import type { Corner } from "./types";
```

Find the existing `useState` block near the top of the component (where `containerHeight` is declared). Add a hover-state line next to it:

```ts
  const [hoveredId, setHoveredId] = useState<string | null>(null);
```

Just below that, add a memoised helper that converts a mouse event to surface-local coords:

```ts
  const eventCoords = useCallback(
    (e: { clientX: number; clientY: number }) => {
      const surface = ref.current?.getBoundingClientRect();
      const offsetX = surface?.left ?? 0;
      const offsetY = surface?.top ?? 0;
      return { x: e.clientX - offsetX, y: e.clientY - offsetY };
    },
    [],
  );
```

Find the `state.rects.map((r, idx) => (` block (currently ~lines 88-160). Replace the rect-wrapper `<div>` and its contents with the following. Take care to preserve the badge + delete button gated by `MIN_RECT_DIMENSION` — they stay, but the wrapper itself now has `pointerEvents: "auto"`, an `onMouseDown` that starts a move, hover handlers, and four new corner handles rendered when this rect is the hovered one.

```tsx
{state.rects.map((r, idx) => (
  <div
    key={r.id}
    data-rect-id={r.id}
    onMouseEnter={() => setHoveredId(r.id)}
    onMouseLeave={() => setHoveredId((prev) => (prev === r.id ? null : prev))}
    onMouseDown={(e) => {
      // Only respond if the mousedown originated on the rect body itself, not on
      // a child (handle, delete-X, badge, note textarea). Children stopPropagation
      // on their own mousedowns.
      if (e.target !== e.currentTarget) return;
      if (e.button !== 0) return;
      e.preventDefault();
      e.stopPropagation();
      const { x: pointerX, y: pointerY } = eventCoords(e);
      dispatch({
        type: "BEGIN_MOVE",
        id: r.id,
        pointerX,
        pointerY,
        viewportW: window.innerWidth,
        viewportH: window.innerHeight,
      });
    }}
    style={{
      position: "absolute",
      left: r.x,
      top: r.y,
      width: r.w,
      height: r.h,
      border: RECT_BORDER,
      background: RECT_FILL,
      boxSizing: "border-box",
      pointerEvents: "auto",
      cursor: "move",
      boxShadow: "-2px 0 0 rgba(255, 0, 60, 0.4), 2px 0 0 rgba(0, 245, 255, 0.4)",
    }}
  >
    {r.w >= MIN_RECT_DIMENSION && r.h >= MIN_RECT_DIMENSION && (
      <>
        <div
          aria-hidden
          style={{
            position: "absolute",
            top: -9,
            left: -13,
            width: 26,
            height: 18,
            borderRadius: 9,
            background: "#ee6633",
            color: "#ffffff",
            font: "bold 11px sans-serif",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            pointerEvents: "none",
            boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
          }}
        >
          {formatLabel(idx)}
        </div>
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
            pointerEvents: "auto",
            boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
          }}
        >
          ×
        </button>
        {hoveredId === r.id && CORNERS.map((corner) => (
          <ResizeHandle
            key={corner}
            corner={corner}
            rectId={r.id}
            dispatch={dispatch}
            eventCoords={eventCoords}
          />
        ))}
      </>
    )}
  </div>
))}
```

- [ ] **Step 2: Add the `ResizeHandle` component and `CORNERS` constant**

Still in `DrawingLayer.tsx`, above the `DrawingLayer` function declaration, add:

```ts
const CORNERS: Corner[] = ["nw", "ne", "sw", "se"];

function ResizeHandle({
  corner,
  rectId,
  dispatch,
  eventCoords,
}: {
  corner: Corner;
  rectId: string;
  dispatch: (action: AnnotationsAction) => void;
  eventCoords: (e: { clientX: number; clientY: number }) => { x: number; y: number };
}) {
  const isDiagNwSe = corner === "nw" || corner === "se";
  const cursor = isDiagNwSe ? "nwse-resize" : "nesw-resize";
  const position: React.CSSProperties = {
    position: "absolute",
    width: 10,
    height: 10,
    background: "#ee6633",
    boxShadow: "0 0 0 1px rgba(0,0,0,0.35)",
    pointerEvents: "auto",
    cursor,
  };
  if (corner === "nw") {
    position.top = -5;
    position.left = -5;
  } else if (corner === "ne") {
    position.top = -5;
    position.right = -5;
  } else if (corner === "sw") {
    position.bottom = -5;
    position.left = -5;
  } else {
    position.bottom = -5;
    position.right = -5;
  }
  return (
    <div
      aria-hidden
      style={position}
      onMouseDown={(e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        e.stopPropagation();
        const { x: pointerX, y: pointerY } = eventCoords(e);
        dispatch({
          type: "BEGIN_RESIZE",
          id: rectId,
          corner,
          pointerX,
          pointerY,
          viewportW: window.innerWidth,
          viewportH: window.innerHeight,
        });
      }}
    />
  );
}
```

Make sure the existing `import type { AnnotationsAction, AnnotationsState } from "./types";` already covers `AnnotationsAction` (it does in the current file). The new `Corner` type was imported at Step 1.

- [ ] **Step 3: Route `onMouseMove`, `onMouseDown` (surface), and `onMouseUp` through `eventCoords`**

Find the `onMouseMove` callback on the drawing surface. Replace it with:

```tsx
  const onMouseMove = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!state.active) return;
      const { x, y } = eventCoords(e);
      dispatch({
        type: "POINTER_TO",
        x,
        y,
        viewportW: window.innerWidth,
        viewportH: window.innerHeight,
      });
    },
    [dispatch, eventCoords, state.active],
  );
```

`BEGIN_CREATE` already converts via `getBoundingClientRect` in the existing `onMouseDown`. Route it through the shared `eventCoords` helper for consistency:

```tsx
  const onMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (e.target !== e.currentTarget) return;
      if (e.button !== 0) return;
      e.preventDefault();
      const { x, y } = eventCoords(e);
      dispatch({ type: "BEGIN_CREATE", id: nextRectId(), x, y });
    },
    [dispatch, eventCoords],
  );
```

The inline `ref.current?.getBoundingClientRect()` calls inside `onMouseDown` and `onMouseMove` are removed — `eventCoords` covers it.

Also update `onMouseUp` to read `state.active` instead of the now-removed `state.draggingId`:

```tsx
  const onMouseUp = useCallback(() => {
    if (!state.active) return;
    dispatch({ type: "END_GESTURE" });
  }, [dispatch, state.active]);
```

(`onMouseUp` may already match this after Task 2 — verify and skip if so.)

- [ ] **Step 4: Run the full test suite to verify no regressions**

Run: `npm test`
Expected: PASS — 30+ tests across 5 files including the new transform + reducer suites.

- [ ] **Step 5: Visual smoke test**

Run the Tauri dev app (one of the worktree's npm scripts; `npm run tauri dev` is the standard path). When the overlay opens via the hotkey:

1. Draw three rectangles of different sizes.
2. Hover each rect — four orange 10×10 corner squares should appear, and the body cursor should change to `move`.
3. Drag a rect's body — it should translate, clamped at viewport edges, without resizing.
4. Drag each of the four corner handles in turn — the rect should resize with the opposite corner pinned, and stop shrinking at roughly 25×25.
5. Try to drag a corner past its opposite corner — it should clamp instead of flipping.
6. Press `Cmd+Z` after a move — the rect snaps back to its previous position.
7. Press `Cmd+Z` after a resize — the rect snaps back to its previous geometry.
8. Press `Cmd+Shift+Enter` to save — open the resulting `~/.glitch-spot/snapshots/<latest>/snapshot.json` and confirm each annotation's `coords` reflects the final (moved/resized) geometry.

If any step misbehaves, stop and investigate — do not paper over a UI bug with style tweaks.

- [ ] **Step 6: Commit**

```bash
git add src/overlay/annotations/DrawingLayer.tsx
git commit -m "feat(overlay): drag-to-move on rect body and corner handles for resize"
```

---

## Self-review checklist

Apply before declaring the plan executed:

- [ ] `npm test` green end-to-end across all 6 task commits.
- [ ] `state.test.ts` covers: BEGIN_MOVE, BEGIN_RESIZE (each corner), POINTER_TO move + resize, clamp-to-viewport, MIN_RECT_DIMENSION on resize, no-flip guarantee, END_GESTURE no-op pop, UNDO move, UNDO resize, UNDO sequence walk-back.
- [ ] `transform.test.ts` covers: clampMoveDelta on each edge, clampResize from each of four corners, MIN_RECT_DIMENSION floor, no-flip past anchor, viewport clamp on the dragged corner.
- [ ] No mentions of `BEGIN_DRAG`, `DRAG_TO`, `END_DRAG`, `draggingId`, or `anchorX`/`anchorY` remain in `src/overlay/` (other than this plan/seed). Use `grep` to confirm.
- [ ] Visual smoke test from Task 6 Step 5 completed.
- [ ] No changes to `packages/electron/`, `src-tauri/`, or the `Toolbar.tsx` / `RectangleNote.tsx` / `DiscardModal.tsx` files.
