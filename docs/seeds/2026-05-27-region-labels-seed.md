# Glitch Spot: Region Labels — Design

Brainstorming output, 2026-05-27. Captures all decisions made before writing the implementation plan.

---

## Goal

Show an auto-derived numeric label (`01`, `02`, `03`, …) on every annotation region — both on the live overlay while the user is drawing and on the exported annotated PNG.

The exported PNG already shows a numbered badge (a filled orange circle with white digits drawn in `src/overlay/annotate.ts`). What's missing is the equivalent indicator on the live overlay, plus a small format unification so live and export agree visually and textually.

---

## Decisions

- **Auto-only, never editable.** Labels are derived from the current array position of the region. There is no rename UI, no schema bump.
- **Format: zero-padded numeric, minimum 2 digits.** `01`, `02`, …, `09`, `10`, …, `99`, `100`. Padding only expands; it never truncates.
- **Compact renumber on delete.** Labels are always `01..N` with no gaps. Deleting region `02` means the old `03` becomes the new `02`. This matches the existing export behavior (which uses array index) and removes the need for a separate `seq` counter.
- **No IPC schema change.** `AnnotationPayload` stays `{ id, x, y, w, h, note }`. Labels are presentation-only on both the live overlay and the canvas renderer.

---

## Visual spec

- **Shape:** filled pill (rounded rectangle), approximately `26 × 18` CSS px, `9` px corner radius. Chosen over a circle because two-character labels (`01`) need a bit more width than the existing 20 px circle comfortably affords.
- **Position:** anchored to the rect's top-left corner, offset so the pill straddles the border — half outside, half inside. Mirrors how the delete-X sits on the top-right corner today.
- **Fill:** `#ee6633` (project accent from `DESIGN.md`).
- **Text:** bold 11 px sans-serif, white (`#ffffff`), horizontally and vertically centered.
- **No chromatic-aberration split.** The split applies to container frames only (per `DESIGN.md`). The badge is an indicator/control-feeling element and stays crisp.
- **Visibility threshold:** only render when the rect's width and height are both ≥ `MIN_RECT_DIMENSION` (25 px). This matches the existing gating for the delete-X and prevents the badge from popping up mid-drag for rectangles that will be discarded as accidental clicks.

---

## Components touched

### `src/overlay/annotations/label.ts` (new)

A tiny module that exports a single function:

```ts
export function formatLabel(idx: number): string {
  return String(idx + 1).padStart(2, "0");
}
```

Both the live overlay and the canvas exporter import this so the formatting rule lives in one place.

### `src/overlay/annotations/DrawingLayer.tsx`

Add a badge `<div>` inside each rect's wrapper, positioned at the top-left corner with the same half-outside offset pattern as the delete-X. Text content is `formatLabel(idx)` where `idx` is the rect's index in `state.rects`. Gated by the same `r.w >= MIN_RECT_DIMENSION && r.h >= MIN_RECT_DIMENSION` check that currently gates the delete-X.

### `src/overlay/annotate.ts`

Swap the existing filled-circle badge for a filled rounded rectangle of the matching pill geometry. Replace `String(idx + 1)` with `formatLabel(idx)`. The pill is sized using the same `scaleFactor` multiplier as the existing badge so it renders correctly on retina captures.

### IPC / payload

Unchanged. `packages/electron/src/types.ts` `AnnotationPayload` stays as-is.

---

## Data flow

- **Live:** `DrawingLayer` maps `state.rects`. For each rect at array index `i`, render badge text `formatLabel(i)`. Undo, delete, and the in-progress drag all re-render naturally; label sequencing falls out of array order.
- **Export:** `annotate.ts` already iterates `annotations.forEach((rect, idx) => …)`. The badge draw call uses `formatLabel(idx)` and the new pill geometry.
- Because both surfaces consume the same `formatLabel`, they cannot diverge.

---

## Edge cases

- **Drag in progress, rect below `MIN_RECT_DIMENSION`:** badge not rendered. Matches today's delete-X gating.
- **Undo a deleted rect:** the reducer reinserts the rect at its original array index, so badges renumber correctly via re-derivation. No state migration needed.
- **100 or more regions:** `padStart(2, "0")` leaves `"100"` as-is. The pill's geometry uses a min-width + horizontal padding so wider labels still center cleanly.
- **Discard modal / Clear:** state resets to `initialState`; the next region naturally starts at `01`.

---

## Testing

- **`src/overlay/annotations/label.test.ts` (new)** — unit tests for `formatLabel`:
  - `formatLabel(0) === "01"`
  - `formatLabel(8) === "09"`
  - `formatLabel(9) === "10"`
  - `formatLabel(99) === "100"`
- **`src/overlay/annotate.test.ts`** — extend existing canvas tests:
  - The first annotation's badge text is `"01"`.
  - The tenth annotation's badge text is `"10"`.
  - The badge is drawn as a pill (rounded rect) at the expected coordinates.
- **`DrawingLayer.tsx`** — no existing component-render tests. Skip adding brittle DOM tests purely for the badge; the shared `formatLabel` unit test plus the canvas test provides the parity guarantee.
- **State reducer:** untouched, so `state.test.ts` does not need new cases.

---

## What's out of scope

- The "overlay window selection" feature named in the worktree branch — handled in a follow-up plan.
- User-editable region names.
- Persisting labels across sessions (snapshots are one-shot).
- Any IPC schema changes.
