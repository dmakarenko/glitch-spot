# Visible-region window picker highlights — auto-seed

> Summary of the brainstorming session captured before drafting
> `docs/plans/2026-05-29-visible-region-highlights.md`. Source of truth for the
> intended behaviour.

## Motivation

When the window picker (⌃⌥⌘J) shows the cyan-checker highlight over a hovered
window, the highlight currently covers the window's full bounding rectangle —
including any portion that is hidden behind a window in front of it. This is
visually wrong: the highlight appears to float above the covering window even
though the user is selecting the window behind.

Dmitry wants the **macOS Screenshot.app** behaviour (⇧⌘4 then Space): the
highlight is clipped to only the visible portion of the target window. Anything
covering the target should appear *over* the highlight, as if the highlight
lived at the target's own z-order.

## Target behaviour (matching macOS Screenshot.app)

1. **Selectability is z-order aware.** Only windows that have at least one
   pixel visible at the cursor are selectable. A fully-occluded window is not
   pickable at all.
2. **Highlight is clipped to visible regions.** The cyan checker only paints
   where the target window is actually visible — never where another window
   covers it. If the user hovers a window whose left half is covered, only the
   right half lights up.

## How the current architecture already supports (1)

`window_picker::hit_test::hit_test` (src-tauri/src/window_picker/hit_test.rs:11)
iterates the window list **front-to-back** and returns the first containing
match. `SCShareableContent::get()` returns windows in z-order; the filter
pipeline (`filter_standard_windows`) preserves input order. So at any cursor
point, hit_test naturally returns the topmost window — exactly the iOS-style
selection semantics.

The only remaining gap: a window that has bounds reachable but **zero visible
pixels** would still appear in the pickable set. After we compute visible
regions, we can drop those.

## How to achieve (2) — visible-region clipping

Tauri's picker overlay windows (`picker-0..picker-7`) sit at the top window
level, so we cannot insert a Tauri window between two arbitrary other apps'
windows. But we don't need to. The visual equivalent is:

> Paint cyan only where the target window's pixels are actually visible. Skip
> the rest. The result is pixel-identical to a "highlight underneath" approach.

For each window in the front-to-back list, the visible region is:

    visible(W_k) = bounds(W_k) − ⋃ bounds(W_i)  for all i < k

Rectangle-minus-rectangle yields up to 4 non-overlapping sub-rectangles (the
top, bottom, left, and right strips around the subtracted intersection).
Iterating produces a `Vec<WindowBounds>` per window. O(N²) worst case — trivial
at N ≈ 30 windows.

Each `PickableWindow` carries its computed `visible_regions: Vec<WindowBounds>`
to the frontend. The React picker renders one `<HighlightRegion>` per region
instead of one rect for the full bounds.

## Limitations (accepted)

- **Rectangular only.** SCWindow `frame()` is a bounding rect; rounded window
  corners over-paint a few pixels. macOS Screenshot.app has the same limit.
- **Frozen at session start.** The window list snapshot is captured once when
  the picker opens. If a window moves during the ~2-3 second session, regions
  go stale. Current behaviour is already frozen on the same boundary.
- **Shadows excluded.** `frame()` excludes window shadow/chrome, so the
  highlight never paints over a shadow pixel. Acceptable.

## Out of scope

- Animation of the highlight transition.
- Hover lag fixes — current 16ms polling cadence is unchanged.
- Picker-window z-order changes (still top-level).
- Hit-test refinement using regions — bounds-based topmost-wins is already
  correct; adding region-precision changes nothing the user can observe.

## Key file pointers

- `src-tauri/src/window_picker/mod.rs` — `PickableWindow` shape (add field here)
- `src-tauri/src/window_picker/macos.rs:23` — `enumerate()` (populate field here)
- `src-tauri/src/window_picker/hit_test.rs` — unchanged behaviour, but the
  `pw()` test helper and fixture conversion need the new field
- `src-tauri/src/window_picker/filter.rs:44` — constructs `PickableWindow`
  (needs the new field on construction)
- `src/picker/types.ts` — TS `PickableWindow` (must mirror Rust)
- `src/picker/Picker.tsx` — `HighlightRect` becomes per-region rendering
- `src/picker/Picker.test.tsx` — fixture extension + multi-region test

## Test commands

- Rust: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- Frontend: `npx vitest run <path>` for single file, `npm test` for all
