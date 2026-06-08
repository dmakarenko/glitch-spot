# Open Image for Annotation — Design

> Seed for a single-PR feature. Plan lives at `docs/plans/2026-06-08-open-file-image.md`.

## Goal

Add a fourth main-menu item that lets the user open any image file for
annotation instead of taking a screenshot. The end result — annotate, redact,
save, clipboard, MCP/share publish — is identical to the existing capture modes.

## Decisions

- **Fit policy: downscale only.** Images larger than the monitor work area are
  scaled down (preserving aspect) to fit; images at or below the work-area size
  are shown at native pixel size, centered. The coordinate scale is therefore
  always `≥ 1.0`, so annotation strokes never thin below native resolution.
- **No global hotkey.** Menu item only — opening a file is a deliberate,
  infrequent action.

## Menu

A fourth item `Open Image…` in the tray's capture group, after
`Capture Window` and before the first separator. Id `open_image`, no
accelerator. Its handler spawns `trigger_open_image(&app)` (mirrors the existing
`capture_*` handlers).

## Flow

1. **File dialog.** Add `tauri-plugin-dialog`. The handler opens a native
   open-panel filtered to image extensions (`png jpg jpeg gif webp bmp`).
   Cancel → no-op, stay Idle.

   - **De-risking spike (do this first).** This is an `LSUIElement` menu-bar app
     with no main window. Native open panels from such apps frequently open
     behind other windows or never take keyboard focus. Before building the rest,
     verify the panel comes to the front and takes focus — this likely needs an
     AppKit activation nudge (`NSApp activateIgnoringOtherApps:` and/or a
     temporary `setActivationPolicy(.regular)` around the dialog). If this is
     broken the feature feels broken regardless of the rest, so it is the first
     thing to prove out.

2. **Decode → frame.** Read the file and decode via the `image` crate (enable
   `jpeg, gif, webp, bmp` features alongside the existing `png`). Re-encode to
   PNG and build a `CapturedFrame { width: iw, height: ih, scale_factor: 1.0,
   png, display_index: <cursor's monitor> }`. The frame metadata stays honest:
   native image dimensions at `1x`. Decode failure → error notification, no
   overlay.

3. **Fit math (pure — TDD seam).**
   `fit_image_rect(work_area_css, (iw, ih)) -> { css_rect, coord_scale, crop_origin }`:
   - `coord_scale = iw / css_rect.w` — native px per overlay CSS px,
     **clamped `≥ 1.0`** (downscale-only; never upscale a small image).
   - `crop_origin = (css_rect.x * coord_scale, css_rect.y * coord_scale)`.
   - `css_rect` is centered in the work area.
   This is the only real arithmetic in the feature; written test-first in
   isolation.

4. **Present.** Cache the frame + `crop_origin` + `coord_scale` in a new
   `AppState.pending_image: Mutex<Option<ImageSession>>`. Present the overlay in
   a new `image` mode (payload = `css_rect`), structurally identical to **window
   mode**: dim surround, frozen content clipped to the rect, chromatic
   glitch-resolve, toolbar, redaction. Unlike frozen mode there is no
   "instant grey veil first" step — the frame is already in hand when we present,
   so the overlay is shown with the frame ready.

5. **Save.** `save_snapshot("image")` reads `pending_image` and calls `build()`
   with `coord_scale` passed as a **new explicit coordinate-scale parameter**
   (defaulting to `frame.scale_factor` for the existing screen modes — a small,
   backward-compatible signature change). Snapshot metadata: `context.screen` =
   native image dims `@1x`, `mode: "image"`. Then persist / clipboard / publish
   exactly like the other modes. Reset `pending_image` to `None` afterward.

## Coordinate model

`build()`'s existing transform `coords.x = a.x * scale - crop_x` already
expresses image-fit. With `scale = coord_scale` and `crop = crop_origin`,
annotations map from overlay CSS pixels into native image pixels. The display
DPR cancels because annotations and the image rect both live in the overlay's
CSS space. `render_annotated_png` / `render_annotations_onto` use the scale
**only** for stroke widths, pill/badge/caption sizing, and font px — never to
re-scale already-absolute coords — so feeding a non-DPR scale value is correct.

## Frontend

- New `'image'` mode in `Overlay.tsx`, reusing the existing window-mode render
  branch (it already does dim surround + clip-rect + glitch resolve + clipped
  `DrawingLayer`). The image-mode payload is a single `CssRect`; the frame
  arrives via the existing `glitch-spot://frame` event / `getFrame()` pull.
- `bridge.ts`: no new methods — `saveSnapshot('image', …)` reuses the existing
  path. The Electron bridge is unaffected (this is a Tauri-tray feature).

## Error handling

- Dialog cancelled → silent no-op, stay Idle.
- Unreadable / undecodable file → notification "Couldn't open image: …", no
  overlay shown, state rolled back to Idle.
- Escape / discard / save behave exactly as in the other modes.

## Testing

- **Rust unit:** `fit_image_rect` (large downscale, small native-centered,
  square / wide / tall, clamp `≥ 1`); `build()` with an explicit coord_scale
  (annotation maps to the correct native px; redaction in image mode covers the
  right region); the decode/dialog-filter helpers where pure.
- Existing `build` / `save` tests stay green via the defaulted coord-scale param.
- **Manual:** run the app; open a small, a large, and a wide image; annotate;
  redact; verify the saved clean / annotated PNGs and the clipboard contents.

## Confirm in-code (not blockers)

- Exact `image`-crate feature flags that decode the target formats without
  bloating the binary.
- The precise AppKit incantation for reliable open-panel focus from an
  `LSUIElement` app.
