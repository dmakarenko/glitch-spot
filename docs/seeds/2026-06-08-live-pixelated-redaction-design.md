# Live pixelated redaction preview

Make the `pixelated` redaction style preview the **real** averaged-block mosaic of the
content underneath it while editing, instead of the fixed grey-checker stand-in used today.
The editing preview becomes faithful to the saved image.

## Problem

When a redaction rect is drawn in `pixelated` style, the overlay shows a fixed grey 12px
checker pattern (`redactionPreviewBg('pixelated')` in `DrawingLayer.tsx`). The baked output,
however, is a coarse mosaic where each block is the **average colour of the source pixels
under it** (`redact.rs::pixelate`, mirrored in `annotate.ts::coverRegions`). So the user
edits against a placeholder that looks nothing like what they will save — they can't judge
whether the chosen region and block coarseness actually obscure the secret until after saving.

The grey checker exists because the comment in `redactionPreviewBg` notes there are "no
live-screen pixels in the DOM". That was a conservative assumption. In reality, **editing
always happens over a resolved screenshot**: a captured frame switches the overlay to
`frozen` mode (or stays `window` mode), and both render the screenshot as a background blob
(`imageUrl`). `live` mode is only the pre-capture transparent state, where no redaction rects
are ever drawn. So the real pixels are present in the DOM and can be sampled.

## Scope

- **`pixelated` only.** `solid` already matches its bake exactly; `branded` is a deterministic
  source-independent pattern whose mosaic stand-in is acceptable and out of scope here.
- **Live re-sampling.** The preview re-computes its averaged blocks as the rect is moved or
  resized, so it always reflects the content currently under it — true "live" pixelation,
  coalesced to one redraw per animation frame.

## Approach

For a `pixelated` redaction rect, replace the mosaic `background` with a `<canvas>` filling
the rect (`inset: 0`, `pointer-events: none`) inside the existing rect `<div>`. The canvas
samples the displayed screenshot at the rect's location and fills averaged 12px blocks using
the same operation the bake performs. The accent border and resize handles stay on top as
existing children of the rect div.

```
Overlay (imageUrl, imageRect) ─▶ DrawingLayer
   • decode imageUrl once → ImageBitmap (source), on imageUrl change
   • per redaction rect, if style === 'pixelated' && source ready:
        ▶ <PixelatedRedactionFill source imageRect rectGeom/>  → <canvas>
     else: transparent interior (border only)
```

`imageRect` is the CSS box the screenshot occupies: the full viewport in `frozen` mode,
`window_rect_css` in `window` mode. One prop covers both `DrawingLayer` call sites.

## Coordinate mapping & block parity

The CSS→source scale is derived from the **displayed image geometry** —
`source.naturalWidth / imageRect.w` — not a separately-passed `scaleFactor`. That ratio equals
the capture's device-pixel ratio, so it is automatically correct in both `frozen` mode (image
fills the viewport) and `window` mode (image fills `window_rect_css`), and it cannot drift
from the capture's own scale.

A rect at CSS `(x, y, w, h)` maps to source pixels:

```
sx = (x - imageRect.x) · scale
sy = (y - imageRect.y) · scale
sw = w · scale
sh = h · scale
```

To match `redact.rs::pixelate` and `annotate.ts::coverRegions` byte-for-byte:

- Block size `= max(2, round(12 · scale))`, in **source physical px**.
- The block grid is **anchored at the rect's top-left**, with partial blocks clamped at the
  right and bottom edges.
- Each block is filled with the **true arithmetic mean** of its source pixels (R, G, B, A
  averaged independently).

The canvas backing store is sized at source-physical resolution (`sw × sh`) and CSS-scaled to
the rect's box, so blocks land pixel-exact and stay crisp on Retina.

## Components & files

1. **`src/overlay/annotations/pixelate.ts`** (new) — pure helper
   `pixelateImageData(src: ImageData, block: number): ImageData` that returns a new `ImageData`
   with each block filled by its mean. Single source of truth for TS block-averaging.

2. **Refactor `src/overlay/annotate.ts`** — the `pixelated` branch of `coverRegions` reads the
   region's `ImageData` once and calls `pixelateImageData`, replacing its inline per-block
   `getImageData` loop. Collapses the two TS copies of the averaging into one and is a minor
   bake speedup. The Rust `redact.rs` stays as-is — separate language, already
   algorithm-identical; both languages remain pinned to the same spec by tests.

3. **`PixelatedRedactionFill`** (new, in `DrawingLayer.tsx`) — renders the rect's `<canvas>`,
   samples the source through `pixelateImageData`, and redraws on geometry change. Redraws are
   coalesced through a single `requestAnimationFrame` tick so pointer-moves firing faster than
   the frame rate don't queue redundant work.

4. **`DrawingLayer`** — gains `sourceImageUrl?: string` and `imageRect?: Rect` props; decodes
   the source once per `imageUrl` (into an `ImageBitmap`); branches a `pixelated` redaction
   rect to `PixelatedRedactionFill` when the source is ready, otherwise a transparent interior.

5. **`Overlay`** — passes `imageUrl` and the image's CSS rect to both `DrawingLayer` call
   sites: the full viewport in `frozen` mode, `window_rect_css` in `window` mode.

## Fallback & edge cases

- **Source not ready** (pre-decode, or `imageUrl` null): the rect's interior is transparent —
  only the accent border shows. The source decodes when the screenshot is presented, well
  before any rect can be drawn, so this gap is effectively never visible; transparent is the
  conceptually cleaner choice and avoids carrying the mosaic stand-in into the pixelated path.
- **Rect extends past the image** (window mode, or off-monitor): sample only the in-bounds
  portion; out-of-image areas render transparent — matching the bake, which only covers
  captured pixels.
- **Source swap** (one screenshot replaced by another): the decode hook clears its canvas to
  null at the start of each decode, so a redaction drawn during the swap window samples nothing
  (transparent) rather than the previous frame's pixels.
- **Overlapping pixelated rects**: each preview rect samples the original screenshot
  independently, but the bake pixelates rects sequentially (a later rect samples an earlier
  rect's already-pixelated overlap). So where two pixelated rects overlap, the saved overlap can
  differ cosmetically from the preview. This is accepted: overlapping pixelated redactions are
  uncommon and the difference is not a content leak (both still obscure the pixels). Matching it
  would require compositing the preview through a shared sequential accumulation canvas.
- **Other styles** (`solid`, `branded`): untouched. `redactionPreviewBg` still serves them.

## Security note

`pixelated` carries the same reversibility caveat as the baked output (averaging is only
practically safe while blocks are coarse relative to glyphs; see the redaction design doc).
This change does not alter that trade-off — it only makes the **preview** show the same
averaged result that is already saved. No new content escapes: the canvas samples the
already-displayed screenshot blob and is `pointer-events: none`; nothing is persisted from it.

## Testing (TDD)

- **`pixelateImageData`** — known two-colour inputs assert exact block means; partial-block
  clamping at the right/bottom edge; block-size formula `max(2, round(12 · scale))`. Mirrors
  the existing `redact.rs` pixelate tests so both languages are pinned to one spec.
- **`annotate.redact.test.ts`** — stays green through the `coverRegions` refactor, proving no
  bake regression.
- **`DrawingLayer`** — with `pixelated` + a source provided, a `<canvas>` renders for redaction
  rects (not the mosaic background); without a source, the interior is transparent (no canvas).

## Out of scope

- Real previews for `branded` (deterministic pattern; current stand-in is acceptable).
- Any change to the bake pipeline's output or the `RedactionStyle` setting/persistence.
- `live`-mode redaction (no rects are drawn there; no screenshot present).
