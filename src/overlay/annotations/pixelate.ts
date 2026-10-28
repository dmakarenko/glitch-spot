// ABOUTME: Pure redaction block-averaging shared by the live preview and the bake.
// ABOUTME: Truncating integer mean per block, mirroring src-tauri/src/snapshot/redact.rs::pixelate.

/** Pixelation block size in CSS px. Coarse so a text glyph spans <1 block. */
export const REDACTION_BLOCK_CSS = 12

/** Block size in physical px for a given scale factor; matches redact.rs::block_px. */
export function redactionBlockPx(scaleFactor: number): number {
  return Math.max(2, Math.round(REDACTION_BLOCK_CSS * scaleFactor))
}

/**
 * Visit each `block`×`block` cell of an RGBA buffer with the cell's bounds and
 * its truncating integer mean (R, G, B, A). The grid is anchored at (0,0);
 * partial cells at the right/bottom edges are clamped. The truncating mean
 * matches src-tauri/src/snapshot/redact.rs::pixelate byte-for-byte. Callers
 * either fill a buffer (the bake) or paint per-cell rects (the live preview),
 * so neither path duplicates the averaging. `stride` (row width of `src` in px)
 * and `offset` (pixel index of the region origin) let a caller average a
 * sub-rectangle of a larger buffer in place.
 */
export function forEachBlockMean(
  src: Uint8ClampedArray,
  width: number,
  height: number,
  block: number,
  visit: (bx: number, by: number, bw: number, bh: number, r: number, g: number, b: number, a: number) => void,
  stride: number = width,
  offset: number = 0,
): void {
  for (let by = 0; by < height; by += block) {
    const byEnd = Math.min(by + block, height)
    for (let bx = 0; bx < width; bx += block) {
      const bxEnd = Math.min(bx + block, width)
      let r = 0, g = 0, b = 0, a = 0, n = 0
      for (let y = by; y < byEnd; y++) {
        for (let x = bx; x < bxEnd; x++) {
          const i = (offset + y * stride + x) * 4
          r += src[i]; g += src[i + 1]; b += src[i + 2]; a += src[i + 3]
          n++
        }
      }
      visit(bx, by, bxEnd - bx, byEnd - by, Math.floor(r / n), Math.floor(g / n), Math.floor(b / n), Math.floor(a / n))
    }
  }
}

/**
 * Average each `block`×`block` cell of an RGBA buffer and refill the cell with
 * that mean, returning a new buffer. Used by the bake (one-shot); the live
 * preview paints from {@link forEachBlockMean} directly to avoid a second
 * full-size allocation per frame.
 */
export function pixelateBlocks(
  src: Uint8ClampedArray,
  width: number,
  height: number,
  block: number,
): Uint8ClampedArray<ArrayBuffer> {
  const out = new Uint8ClampedArray(src.length)
  forEachBlockMean(src, width, height, block, (bx, by, bw, bh, r, g, b, a) => {
    for (let y = by; y < by + bh; y++) {
      for (let x = bx; x < bx + bw; x++) {
        const i = (y * width + x) * 4
        out[i] = r; out[i + 1] = g; out[i + 2] = b; out[i + 3] = a
      }
    }
  })
  return out
}
