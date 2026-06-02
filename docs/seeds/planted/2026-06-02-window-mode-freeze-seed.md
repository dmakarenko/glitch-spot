# Window-mode freeze — design

> Part of initiative: docs/initiatives/glitch-spot/INITIATIVE.md (enhancement to the shipped window mode; single-PR, not a new stage)

## Problem

Window mode (`Ctrl+Opt+Cmd+J`) currently annotates on the **live** window seen through
the transparent overlay: it dims the surrounding screen, paints a faint gray-white checker
over the selected window's rect, and only captures the isolated window image at **save**
time (`save_snapshot("window")` → hide overlay → 120ms sleep → `capture_window`).

Two problems with annotating on the live window:

1. **You see whatever is actually on screen at the window's bounds** — including any windows
   overlapping the selected one, and any live changes/animations beneath the overlay.
2. The save-time recapture is laggy and can **drift**: if the window moved, resized, or
   changed between pick and save, the saved PNG differs from what was annotated.

## Goal

In window mode, present a **frozen, isolated screenshot of just the selected window**,
above everything, and annotate on that. The capture already isolates the single window
(`capture_window` builds the filter with `.with_window(&window)` — desktop-independent
single-window capture), so "above all other windows / looks much better" is free once we
display the captured frame instead of the live view.

This mirrors monitor freeze (`Ctrl+Opt+Cmd+H`), which already captures into
`state.latest_frame` and reuses that cached frame at save time.

## Decisions

- **Freeze replaces live.** The live-through-transparency view and the checker placeholder
  are removed. Frozen capture is the only window-mode rendering.
- **Resolve glitch only.** No pending grey veil / scanline shimmer (window capture is fast).
  Paint the frozen window image and play the chromatic-shake resolve when it lands.
- **Save the frozen frame.** If the selected window closes after it was frozen, the snapshot
  still saves from the cached frame (you froze it, you keep it). This flips the old
  "close selected window mid-annotation → no snapshot written" behavior.

## Design

### Backend (`src-tauri/src/lib.rs`)

- **`picker_select`**: **show first, then capture** (mirrors monitor freeze's
  `show_frozen_pending` → capture). After resolving the window + display, call
  `overlay::show_window_mode` (dim surround + transparent rect, no checker — live window shows
  through during the brief gap), *then* capture the isolated window
  (`capture::capture_window(&app, window_id)`), store it in `state.latest_frame`, and
  `overlay::push_frame` it. The visible overlay does not pollute the grab because
  `capture_window` filters with `.with_window(&window)` (single-window isolation). Capture
  failure aborts the pick with the existing "Selected window closed — annotations discarded"
  notification, hides the overlay, and resets to idle — this becomes the only place a "window
  gone" failure can occur. `push_frame` necessarily runs after `show_window_mode`'s
  `eval('window')`, so the frontend's `resetFrames()` cannot wipe the pushed frame.
- **`save_snapshot` `"window"` branch**: drop the recapture, the `overlay::hide` + 120ms
  sleep, and the window-closed error path. Reuse `state.latest_frame` (as the `"frozen"`
  branch already does). Keep the existing crop-origin computation and `context.window`
  attachment unchanged.

Net: **one capture total** (at pick), down from two (pick-time display + save-time recapture).

### Frontend (`src/overlay/Overlay.tsx`)

- Window-mode render path (`mode === 'window'`):
  - **Remove** the gray-white checker `<div>`.
  - **Keep** the dimmed surround (the spotlight `boxShadow: '0 0 0 100vmax rgba(0,0,0,0.40)'`
    at the window rect).
  - Render the pushed frame image in a box positioned/clipped to `window_rect_css`.
  - During the sub-100ms capture gap (pick click → frame arrives) the rect stays transparent
    so the live window shows through; when the frozen isolated capture lands it resolves in
    with the existing chromatic-shake animation, scoped to the window box. This gives a clean
    "messy live → clean frozen" transition.
  - `DrawingLayer` still clips to `window_rect_css`; the Toolbar, DiscardModal, and
    outside-click behavior are unchanged.

### Data flow

```
pick window
  → backend show_window_mode (dim + transparent rect, no checker)
  → backend capture_window (isolated, window-sized)
  → cache in state.latest_frame + push_frame to overlay
  → frontend resolves frozen image into the rect (chromatic shake)
  → user annotates (drawing clipped to rect)
  → save reuses state.latest_frame → crop → annotated PNG + snapshot JSON
```

### Behavior change to document

`docs/initiatives/glitch-spot/2026-05-27-window-mode-smoke.md`: the
"Close selected window mid-annotation → notification fires, no snapshot written" expectation
changes to "save still produces the frozen capture; no recapture occurs." The capture-failure
notification now fires at **pick** time (window already gone when you click it), not at save.

## Testing (TDD)

- **Rust (pure-function TDD).** The frame-resolution policy for window save is extracted into a
  pure helper `resolve_window_save_frame(session, latest_frame) -> Result<(CapturedFrame,
  (f32, f32)), String>` that owns `display_index` assignment + crop-origin computation. Tested
  with real data (no `AppHandle` mock): reuses the cached frame; computes crop from window/
  display bounds; errors when `latest_frame` is `None`. (`CapturedFrame` round-trips and
  `AnnotatingSession` is already constructed in `picker_session` tests.)
- **Frontend (vitest, static seam).** jsdom does **not** fire `img.onload` on `img.src=`
  (verified), so the actual image-render + chromatic resolve is **smoke-verified**, not
  unit-tested. TDD covers the statically-assertable behavior:
  - Window mode renders a frozen-frame container at `window_rect_css` with `overflow: hidden`
    (`data-testid="window-frozen-frame"`).
  - The gray-white checker `<div>` is gone.
  - The spotlight dim is preserved (existing test).
- **Not unit-testable (smoke only):** `picker_select`'s capture+cache+push wiring (needs a
  real screen capture / `AppHandle`); the frozen image actually rendering and the resolve
  animation playing. Covered by the manual window-mode smoke checklist.
- Existing Rust suite (188) + vitest suite (115) stay green.

## Out of scope

- The broader "Save in freeze/annotation mode is very laggy" perf side quest
  (`20260602-110255`) — this change incidentally removes the window-mode save recapture but
  does not chase full-resolution encode/persist profiling.
- The "window between displays → dim on wrong display" bug (`20260531-210026`).
