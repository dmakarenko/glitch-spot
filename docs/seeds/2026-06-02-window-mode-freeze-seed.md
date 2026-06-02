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

- **`picker_select`**: after resolving the window + display, capture the isolated window
  (`capture::capture_window(&app, window_id)`), store it in `state.latest_frame`, and
  `overlay::push_frame` it to the overlay. Capture failure here aborts the pick with the
  existing "Selected window closed — annotations discarded" notification and resets to idle —
  this becomes the only place a "window gone" failure can occur.
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
  → backend capture_window (isolated, window-sized)
  → cache in state.latest_frame + push_frame to overlay
  → frontend paints frozen image at window_rect_css, chromatic-resolve
  → user annotates (drawing clipped to rect)
  → save reuses state.latest_frame → crop → annotated PNG + snapshot JSON
```

### Behavior change to document

`docs/initiatives/glitch-spot/2026-05-27-window-mode-smoke.md`: the
"Close selected window mid-annotation → notification fires, no snapshot written" expectation
changes to "save still produces the frozen capture; no recapture occurs." The capture-failure
notification now fires at **pick** time (window already gone when you click it), not at save.

## Testing (TDD)

- **Rust**
  - `picker_select` populates `state.latest_frame` with the captured window frame.
  - `save_snapshot("window")` reuses the cached frame and does **not** recapture or sleep;
    crop-origin + `context.window` still correct.
  - Pick-time capture failure resets to idle and surfaces the notification path.
- **Frontend (vitest)**
  - Window mode renders the pushed frame image (no checker element present).
  - `DrawingLayer` remains clipped to `window_rect_css`.
  - Existing multi-display / region / save-JSON window-mode tests stay green.

## Out of scope

- The broader "Save in freeze/annotation mode is very laggy" perf side quest
  (`20260602-110255`) — this change incidentally removes the window-mode save recapture but
  does not chase full-resolution encode/persist profiling.
- The "window between displays → dim on wrong display" bug (`20260531-210026`).
