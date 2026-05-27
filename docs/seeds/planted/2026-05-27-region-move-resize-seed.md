# Glitch Spot: Region Move & Resize — Design

Brainstorming output, 2026-05-27. Captures all decisions made before writing the implementation plan.

---

## Goal

Let the user adjust any rectangle drawn during the current overlay session — by dragging the rect body to move it, or by dragging a corner handle to resize it — before saving the snapshot. The current flow only supports drawing a rect from scratch; once committed, a rect's geometry is immutable until delete + redraw.

---

## Decisions

- **Scope: all rectangles in the current session.** Any rect the user has drawn is movable and resizable until `CLEAR` (save or discard) wipes state. No "selected" mode — geometry edits work in place.
- **Affordance: hover-revealed handles.** The four corner handles are not rendered by default. They appear when the cursor enters a rect (or its handle area) and disappear when the cursor leaves. The rect border itself is unchanged from today.
- **Handles: four corners only** (NW, NE, SW, SE). Edge-midpoint and single-corner alternatives were rejected as overkill or unnecessarily restrictive for a screenshot annotation tool.
- **Constraints:** rects are clamped to the overlay viewport during both move and resize, and resize cannot shrink a rect below `MIN_RECT_DIMENSION` (25 px) or flip a corner through its anchor.
- **Undo:** each move and each resize is one undo step. The history entry stores the previous geometry and is pushed at `BEGIN_*`, then rolled back on gesture-end if no movement occurred.
- **No IPC change.** Move/resize lives entirely inside the React reducer; the snapshot save flow already serializes the rect's final `x/y/w/h`.

---

## Interaction model

On `mousedown`, the hit target chooses the gesture:

| Hit target | Gesture |
|---|---|
| Empty drawing surface | Start a new rect (existing flow, unchanged) |
| An existing rect's body | Start **move** of that rect |
| An existing rect's corner handle | Start **resize** of that rect, anchored to the opposite corner |

Cursor feedback signals the affordance:
- Cursor over a rect's body → `move`
- Cursor over NW or SE handle → `nwse-resize`
- Cursor over NE or SW handle → `nesw-resize`
- Cursor on empty drawing surface → existing `crosshair`

**Known trade-off:** because rect bodies now intercept clicks, drawing a *new* rect inside an existing rect is not possible. Accepted — the existing rect can be resized smaller, moved out of the way, or deleted first.

---

## State model changes

Today's `draggingId: string | null` only encodes "a rect is being created". Generalise to a tagged gesture so the reducer knows what the next `POINTER_TO` should do:

```ts
type Corner = 'nw' | 'ne' | 'sw' | 'se';

type ActiveGesture =
  | { kind: 'create'; id: string }
  | { kind: 'move';
      id: string;
      pointerStart: { x: number; y: number };
      rectStart:    { x: number; y: number };
    }
  | { kind: 'resize';
      id: string;
      corner: Corner;
      anchor: { x: number; y: number };
    };

type AnnotationsState = {
  rects: Rect[];
  history: HistoryEntry[];
  active: ActiveGesture | null;
  lastCreatedId: string | null;
};
```

The `Rect.anchorX/anchorY` fields used today for the creation drag are retired — the anchor lives on the gesture struct instead. `Rect` becomes `{ id, x, y, w, h, note, label }` (label already lives there per the recent region-labels feature).

---

## Actions

```ts
type AnnotationsAction =
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

`BEGIN_CREATE` is the rename of today's `BEGIN_DRAG`. `POINTER_TO` is the rename of today's `DRAG_TO`. `END_GESTURE` is the rename of today's `END_DRAG`. The reducer dispatches inside `POINTER_TO` and `END_GESTURE` based on `state.active.kind`.

---

## History entries

Two new kinds. Both store the *previous* geometry — undo restores it directly.

```ts
type HistoryEntry =
  | { kind: 'create';   id: string }
  | { kind: 'delete';   rect: Rect; index: number }
  | { kind: 'noteEdit'; id: string; previousNote: string }
  | { kind: 'move';     id: string; previousX: number; previousY: number }
  | { kind: 'resize';   id: string; previous: { x: number; y: number; w: number; h: number } };
```

The `move` / `resize` entries are pushed at the matching `BEGIN_*`. If `END_GESTURE` fires without any intervening `POINTER_TO` (mousedown then mouseup with no movement), the just-pushed entry is popped from history so undo doesn't have to no-op through it.

`UNDO` extends with two cases:
- `move`: set the rect's `x` / `y` back to `previousX` / `previousY`.
- `resize`: set the rect's `x`, `y`, `w`, `h` back to `previous`.

---

## Constraint helpers (pure)

```ts
function clampMoveDelta(
  rect: { x: number; y: number; w: number; h: number },
  deltaX: number,
  deltaY: number,
  viewport: { w: number; h: number }
): { x: number; y: number };

function clampResize(
  corner: Corner,
  anchor: { x: number; y: number },
  pointer: { x: number; y: number },
  viewport: { w: number; h: number }
): { x: number; y: number; w: number; h: number };
```

`clampMoveDelta` keeps `x >= 0`, `y >= 0`, `x + w <= viewport.w`, `y + h <= viewport.h`.

`clampResize` keeps the dragged corner on the same side of the anchor (no flip), enforces `w >= MIN_RECT_DIMENSION && h >= MIN_RECT_DIMENSION`, and clamps the resulting rect inside the viewport.

Both helpers are pure — unit-tested in isolation.

---

## Components touched

### `src/overlay/annotations/state.ts`

Rename existing actions (`BEGIN_DRAG` → `BEGIN_CREATE`, `DRAG_TO` → `POINTER_TO`, `END_DRAG` → `END_GESTURE`). Add `BEGIN_MOVE`, `BEGIN_RESIZE`. Replace `draggingId` with `active: ActiveGesture | null`. Add reducer branches for move and resize. Add the two new `HistoryEntry` kinds and corresponding `UNDO` branches. Retire `Rect.anchorX/anchorY`.

### `src/overlay/annotations/transform.ts` (new)

`clampMoveDelta`, `clampResize`, and a small `Corner` helper map. Pure functions. Imported by `state.ts`; not consumed at the UI layer (the reducer is the single point where constraints apply).

### `src/overlay/annotations/DrawingLayer.tsx`

- Rect wrapper `<div>` gains `pointerEvents: 'auto'` (was `'none'`), an `onMouseDown` that dispatches `BEGIN_MOVE`, and `onMouseEnter` / `onMouseLeave` that toggle a `hoveredId` piece of local state.
- When `hoveredId === rect.id`, render four corner-handle `<div>`s. Each handle is 10×10 px positioned with `top` / `left` (or `top` / `right`, etc.) of `-5` from its corner so it straddles the rect edge. Fill `#ee6633`, no chromatic split (handles are interactive controls per DESIGN.md). Each handle has its own `onMouseDown` that dispatches `BEGIN_RESIZE` with the appropriate `corner`.
- The drawing surface's `onMouseDown` keeps the `e.target !== e.currentTarget` guard so child-element mousedowns don't double-fire as new-rect creation.
- Cursor styles applied inline: `cursor: 'move'` on the body, `cursor: 'nwse-resize'` or `'nesw-resize'` on the handles.

### `src/overlay/annotations/types.ts`

Update the type definitions to match the new state/action shapes above. Drop `anchorX` / `anchorY` from `Rect`.

### `src/overlay/Overlay.tsx`

Existing `BEGIN_DRAG` / `DRAG_TO` / `END_DRAG` references rename to the new action names. No other change.

---

## Data flow

Mousedown on a rect body:
1. `DrawingLayer` rect-wrapper `onMouseDown` → dispatch `BEGIN_MOVE { id, pointerX, pointerY }`.
2. Reducer sets `state.active = { kind: 'move', id, pointerStart, rectStart }` and pushes `{ kind: 'move', id, previousX, previousY }` onto history.
3. Mousemove → drawing surface `onMouseMove` → dispatch `POINTER_TO { x, y }`. Reducer reads `state.active.kind === 'move'`, computes new position via `clampMoveDelta`, updates the rect in place.
4. Mouseup → `END_GESTURE`. Reducer clears `state.active`. If the rect's geometry equals `previousX/previousY`, pop the history entry.

Resize follows the same pattern with `clampResize` and the corner-anchor data.

---

## Testing

State reducer is pure; the new logic gets unit tests in `state.test.ts`:

- `BEGIN_MOVE` + `POINTER_TO` + `END_GESTURE` translates the rect by the delta, clamped to viewport.
- `BEGIN_MOVE` + immediate `END_GESTURE` (no movement) leaves history unchanged.
- `BEGIN_RESIZE` from each of the four corners + `POINTER_TO` extends or shrinks the rect with the opposite corner pinned.
- `BEGIN_RESIZE` + `POINTER_TO` past the anchor stops at `MIN_RECT_DIMENSION` — no flip.
- Viewport-edge clamping: `clampMoveDelta` and `clampResize` exercised with a battery of cases.
- `UNDO` after a move restores `x` / `y`. `UNDO` after a resize restores all four geometry fields.
- A sequence `move → resize → UNDO → UNDO` restores the original geometry.

`transform.ts` helpers get their own pure unit tests next to the reducer tests.

`DrawingLayer.tsx` rendering and pointer event wiring are manual-verified — consistent with how the existing badge and delete-X are validated. The reducer + helper coverage gives high confidence in the math.

---

## Edge cases

- **Click on a corner of rect A that overlaps rect B's body.** Corner handles render later in the React tree than rect bodies, so their `onMouseDown` fires first. They `stopPropagation`. The body's handler never runs.
- **Cursor leaves the viewport mid-drag.** The drawing surface's existing `onMouseLeave={onMouseUp}` triggers `END_GESTURE`. Clamp helpers have already constrained the values.
- **Note textarea opens over a rect's corner.** The textarea already has `onMouseDown` that stops propagation, so clicking inside the textarea never starts a move or a resize.
- **Two rects fully overlap.** The top-most (later in `state.rects`) wins for body mousedown. Predictable, mirrors z-order behavior elsewhere.
- **Mousedown on a rect body, mouseup without movement.** History entry is popped at `END_GESTURE`; undo doesn't have to walk through an empty step.

---

## What's out of scope

- Keyboard nudge (arrow keys to move 1 px / 10 px with shift).
- Multi-select and group move/resize.
- Rotation.
- Snapping to other rects' edges or to the viewport edges.
- Aspect-ratio lock (e.g., Shift to preserve ratio during resize).
