# Live Pixelated Redaction Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `pixelated` redaction style preview the real averaged-block mosaic of the content underneath it while editing, instead of the fixed grey-checker stand-in, so the editing preview looks identical to the saved image.

**Architecture:** Editing always happens over a resolved screenshot (`frozen`/`window` mode renders it as a blob `imageUrl`). For a `pixelated` redaction rect we replace the mosaic CSS background with a `<canvas>` that samples that screenshot under the rect and fills averaged 12px blocks using the exact algorithm the bake uses. A single shared TS helper (`pixelateBlocks`, truncating integer mean to match `redact.rs`) drives both the live preview and the Electron `annotate.ts` bake, so preview output equals final output on both platforms.

**Tech Stack:** React + TypeScript, Vitest (jsdom), Canvas 2D. Spec: `docs/seeds/2026-06-08-live-pixelated-redaction-design.md`.

---

## Coordinate parity (verified against the bake)

The bake caller (`src-tauri/src/lib.rs::save_snapshot` → `snapshot::build::build`) maps each CSS rect to a physical `Region` as `Region.x = round(rect.x · scale_factor − crop_x)`, block size `round(12 · scale_factor)`, grid anchored at `Region.x`. The preview must reproduce this. Verified per mode:

- **Frozen** (`crop_origin = None` → 0): base = full active-monitor capture; displayed blob fills the viewport. Preview `imageRect = {0,0,innerWidth,innerHeight}`, so `imageRect.x·scale = 0 = crop_x`, and `scale = source.width / innerWidth = DPR = scale_factor`. **Exact match.**
- **Window** (`crop_origin = (window.bounds.x − display.x, …)`, physical px): base = the cached isolated **window** capture (size = `window_rect_css × scale_factor`); the displayed blob is that same capture filling `window_rect_css`. Preview `scale = source.width / window_rect_css.w = scale_factor` (by construction, so fractional DPR is handled — scale is *derived*, never assumed integer), and `imageRect.x·scale = window_rect_css.x·scale_factor = crop_x`. **Match**, modulo a possible ±1 physical-px grid-origin rounding in fractional-DPR window mode (negligible).
- **Live**: no blob in the DOM (transparent pre-capture state; no rects drawn there) → `sourceImageUrl` undefined → transparent fallback. Out of scope, safe.

This is why the preview derives `scale` from displayed geometry (`source.width / imageRect.w`) rather than threading a `scaleFactor`: it reproduces the bake's `scale_factor` in every mode without assuming the capture geometry.

---

## File Structure

- **Create** `src/overlay/annotations/pixelate.ts` — pure block-averaging helper + block-size formula. Single source of truth for TS pixelation.
- **Create** `src/overlay/annotations/pixelate.test.ts` — unit tests for the helper.
- **Create** `src/overlay/annotations/useDecodedSource.ts` — React hook decoding a blob URL into an offscreen sampling canvas, once per URL.
- **Create** `src/overlay/annotations/useDecodedSource.test.ts` — hook tests (mocked `Image`/`canvas`).
- **Modify** `src/overlay/annotate.ts` — `coverRegions` pixelated branch reads the region once and delegates to `pixelateBlocks`; drop the local `BLOCK_CSS` dup.
- **Modify** `src/overlay/annotate.redact.test.ts` — add a pixelated integration test proving the averaged mean is written back.
- **Modify** `src/overlay/annotations/DrawingLayer.tsx` — `PixelatedRedactionFill` component; new `sourceImageUrl`/`imageRect` props; branch pixelated rects to the canvas.
- **Modify** `src/overlay/annotations/DrawingLayer.test.tsx` — assert pixelated rects render no mosaic background; branded/solid unchanged.
- **Modify** `src/overlay/Overlay.tsx` — pass `sourceImageUrl` + `imageRect` to both `DrawingLayer` call sites.

---

## Task 1: Shared block-averaging helper

**Files:**
- Create: `src/overlay/annotations/pixelate.ts`
- Test: `src/overlay/annotations/pixelate.test.ts`

- [ ] **Step 1: Write the failing tests**

```typescript
// ABOUTME: Tests for the shared redaction block-averaging helper.
// ABOUTME: Pins the truncating mean, edge clamping, and block-size formula to redact.rs.
import { describe, it, expect } from 'vitest'
import { pixelateBlocks, redactionBlockPx, REDACTION_BLOCK_CSS } from './pixelate'

// Build an RGBA buffer width*height with a per-pixel color from `color(x,y)`.
function buffer(width: number, height: number, color: (x: number, y: number) => [number, number, number, number]) {
  const buf = new Uint8ClampedArray(width * height * 4)
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4
      const [r, g, b, a] = color(x, y)
      buf[i] = r; buf[i + 1] = g; buf[i + 2] = b; buf[i + 3] = a
    }
  }
  return buf
}

describe('redactionBlockPx', () => {
  it('is 12 at scale 1', () => {
    expect(REDACTION_BLOCK_CSS).toBe(12)
    expect(redactionBlockPx(1)).toBe(12)
  })
  it('scales and rounds with the scale factor', () => {
    expect(redactionBlockPx(2)).toBe(24)
    expect(redactionBlockPx(1.5)).toBe(18)
  })
  it('never drops below 2', () => {
    expect(redactionBlockPx(0.01)).toBe(2)
  })
})

describe('pixelateBlocks', () => {
  it('fills a single full block with the truncated mean of its pixels', () => {
    // 2x1 block: red(200,0,0) and (0,0,0) → mean (100,0,0).
    const out = pixelateBlocks(buffer(2, 1, (x) => (x === 0 ? [200, 0, 0, 255] : [0, 0, 0, 255])), 2, 1, 12)
    expect([out[0], out[1], out[2], out[3]]).toEqual([100, 0, 0, 255])
    expect([out[4], out[5], out[6], out[7]]).toEqual([100, 0, 0, 255])
  })

  it('truncates rather than rounds the mean (matches redact.rs)', () => {
    // Three pixels: 0, 0, 2 → mean 0.666… → truncates to 0 (round would give 1).
    const out = pixelateBlocks(buffer(3, 1, (x) => (x === 2 ? [2, 2, 2, 255] : [0, 0, 0, 255])), 3, 1, 12)
    expect(out[0]).toBe(0)
  })

  it('averages each block independently and clamps partial edge blocks', () => {
    // width 3, block 2 → columns [0,1] form one block, column [2] a partial block.
    // Row of values 60, 60, 240 → block A mean 60, block B (partial) mean 240.
    const out = pixelateBlocks(buffer(3, 1, (x) => (x < 2 ? [60, 60, 60, 255] : [240, 240, 240, 255])), 3, 1, 2)
    expect(out[0]).toBe(60)      // x=0
    expect(out[4]).toBe(60)      // x=1
    expect(out[8]).toBe(240)     // x=2 (its own partial block)
  })

  it('returns a new buffer of the same length', () => {
    const src = buffer(4, 4, () => [10, 20, 30, 255])
    const out = pixelateBlocks(src, 4, 4, 2)
    expect(out).not.toBe(src)
    expect(out.length).toBe(src.length)
  })
})
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/overlay/annotations/pixelate.test.ts`
Expected: FAIL — `pixelate.ts` does not exist / exports undefined.

- [ ] **Step 3: Write the helper**

```typescript
// ABOUTME: Pure redaction block-averaging shared by the live preview and the bake.
// ABOUTME: Truncating integer mean per block, mirroring src-tauri/src/snapshot/redact.rs::pixelate.

/** Pixelation block size in CSS px. Coarse so a text glyph spans <1 block. */
export const REDACTION_BLOCK_CSS = 12

/** Block size in physical px for a given scale factor; matches redact.rs::block_px. */
export function redactionBlockPx(scaleFactor: number): number {
  return Math.max(2, Math.round(REDACTION_BLOCK_CSS * scaleFactor))
}

/**
 * Average each `block`×`block` cell of an RGBA buffer and refill the cell with
 * that mean. The grid is anchored at (0,0); partial cells at the right/bottom
 * edges are clamped. Uses a truncating integer mean to match
 * src-tauri/src/snapshot/redact.rs::pixelate byte-for-byte. Returns a new buffer.
 */
export function pixelateBlocks(
  src: Uint8ClampedArray,
  width: number,
  height: number,
  block: number,
): Uint8ClampedArray {
  const out = new Uint8ClampedArray(src.length)
  for (let by = 0; by < height; by += block) {
    const byEnd = Math.min(by + block, height)
    for (let bx = 0; bx < width; bx += block) {
      const bxEnd = Math.min(bx + block, width)
      let r = 0, g = 0, b = 0, a = 0, n = 0
      for (let y = by; y < byEnd; y++) {
        for (let x = bx; x < bxEnd; x++) {
          const i = (y * width + x) * 4
          r += src[i]; g += src[i + 1]; b += src[i + 2]; a += src[i + 3]
          n++
        }
      }
      const rr = Math.floor(r / n)
      const gg = Math.floor(g / n)
      const bb = Math.floor(b / n)
      const aa = Math.floor(a / n)
      for (let y = by; y < byEnd; y++) {
        for (let x = bx; x < bxEnd; x++) {
          const i = (y * width + x) * 4
          out[i] = rr; out[i + 1] = gg; out[i + 2] = bb; out[i + 3] = aa
        }
      }
    }
  }
  return out
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/overlay/annotations/pixelate.test.ts`
Expected: PASS — all cases green.

- [ ] **Step 5: Commit**

```bash
git add src/overlay/annotations/pixelate.ts src/overlay/annotations/pixelate.test.ts
git commit -m "feat: shared truncating block-averaging helper for redaction"
```

---

## Task 2: Refactor annotate.ts to use the shared helper

**Files:**
- Modify: `src/overlay/annotate.ts:8-67` (constant + `coverRegions` pixelated branch)
- Test: `src/overlay/annotate.redact.test.ts` (add a pixelated case)

- [ ] **Step 1: Write the failing test**

Add this test inside the existing `describe('redactClean', ...)` block in `src/overlay/annotate.redact.test.ts`. It backs the fake canvas with a real RGBA store so the pixelated branch is exercised end-to-end and the truncated mean is asserted.

```typescript
  it('pixelated writes the truncated block mean back to the region', async () => {
    // 4x4 source image, scale 1 → one 12px block covers the whole 4x4 region.
    const W = 4, H = 4
    const store = new Uint8ClampedArray(W * H * 4)
    // Half black, half (4,4,4,255): 8 px of 0 + 8 px of 4 → mean 2 (floor of 2.0).
    for (let i = 0; i < store.length; i += 4) {
      const px = i / 4
      const v = px < 8 ? 0 : 4
      store[i] = v; store[i + 1] = v; store[i + 2] = v; store[i + 3] = 255
    }
    let written: Uint8ClampedArray | null = null
    const fakeCtx = {
      drawImage: () => {},
      set fillStyle(_v: string) {}, get fillStyle() { return '#000' },
      fillRect: () => {},
      beginPath: () => {}, roundRect: () => {}, fill: () => {},
      getImageData: (x: number, y: number, w: number, h: number) => {
        const data = new Uint8ClampedArray(w * h * 4)
        for (let yy = 0; yy < h; yy++)
          for (let xx = 0; xx < w; xx++) {
            const si = ((y + yy) * W + (x + xx)) * 4
            const di = (yy * w + xx) * 4
            data[di] = store[si]; data[di + 1] = store[si + 1]
            data[di + 2] = store[si + 2]; data[di + 3] = store[si + 3]
          }
        return { data, width: w, height: h }
      },
      putImageData: (img: { data: Uint8ClampedArray }) => { written = img.data },
    }
    const fakeCanvas = { width: 0, height: 0, getContext: () => fakeCtx, toDataURL: () => 'data:image/png;base64,AAAA' }
    ;(document as unknown as { createElement: (t: string) => unknown }).createElement = (t: string) =>
      t === 'canvas' ? fakeCanvas : pristineCreateElement(t)
    ;(globalThis as unknown as { ImageData: unknown }).ImageData =
      class { data: Uint8ClampedArray; width: number; height: number
        constructor(d: Uint8ClampedArray, w: number, h: number) { this.data = d; this.width = w; this.height = h } }
    class FakeImage { naturalWidth = W; naturalHeight = H; onload: (() => void) | null = null; set src(_s: string) { this.onload?.() } }
    ;(globalThis as unknown as { Image: unknown }).Image = FakeImage as unknown

    await redactClean('blob:x', [{ id: 'r', x: 0, y: 0, w: 4, h: 4, note: '', kind: 'redaction' }], 1, 'pixelated')

    expect(written).not.toBeNull()
    // Every pixel of the single block is the truncated mean = 2.
    expect((written as unknown as Uint8ClampedArray)[0]).toBe(2)
  })
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/overlay/annotate.redact.test.ts`
Expected: FAIL — current per-block loop rounds (mean 2.0 floors to 2 here so value matches, but the loop calls `getImageData` per block with no `putImageData`, so `written` stays null) → `expect(written).not.toBeNull()` fails.

- [ ] **Step 3: Refactor `coverRegions` to use the helper**

In `src/overlay/annotate.ts`, replace the import/constant and the pixelated branch.

Change the imports + constants block (lines 4-12) so the local `BLOCK_CSS` is gone and the helper is imported:

```typescript
import type { RedactionStyle } from './bridge'
import { annotationLabels } from './annotations/label'
import { wrapNote } from './annotations/wrapNote'
import { pixelateBlocks, redactionBlockPx } from './annotations/pixelate'

const COVER_DARK = '#0c0c0c' // DESIGN.md background — matches redact.rs COVER_DARK
const SOLID_COVER = '#241f1b' // warm dark grey solid cover — matches redact.rs SOLID_COVER
const ACCENT = '#ee6633'     // matches redact.rs ACCENT
const ACCENT_DIM = '#78341a' // matches redact.rs ACCENT_DIM
```

Update the block computation at the top of `coverRegions` (was `const block = Math.max(2, Math.round(BLOCK_CSS * scaleFactor))`):

```typescript
  const block = redactionBlockPx(scaleFactor)
```

Replace the pixelated branch (the `// pixelated: average each block…` loop) with a single read → average → write:

```typescript
    // pixelated: average each block from the source and refill it. Shared with
    // the live editing preview via pixelateBlocks so both look identical.
    const region = ctx.getImageData(x, y, w, h)
    const averaged = pixelateBlocks(region.data, w, h, block)
    ctx.putImageData(new ImageData(averaged, w, h), x, y)
```

(Keep the `import type { AnnotationPayload }` line as-is above these.)

- [ ] **Step 4: Run the test suite to verify it passes**

Run: `npx vitest run src/overlay/annotate.redact.test.ts`
Expected: PASS — the new pixelated test and the existing solid/branded/numbering tests all green.

- [ ] **Step 5: Commit**

```bash
git add src/overlay/annotate.ts src/overlay/annotate.redact.test.ts
git commit -m "refactor: route annotate pixelation through shared helper"
```

---

## Task 3: Decode-source hook

**Files:**
- Create: `src/overlay/annotations/useDecodedSource.ts`
- Test: `src/overlay/annotations/useDecodedSource.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// ABOUTME: Tests for useDecodedSource — decodes a blob URL into an offscreen
// ABOUTME: sampling canvas, once per URL, null until ready and when url is undefined.
// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest'
import { render, cleanup, waitFor } from '@testing-library/react'
import { useDecodedSource } from './useDecodedSource'

afterEach(cleanup)

function Probe({ url, onValue }: { url?: string; onValue: (c: HTMLCanvasElement | null) => void }) {
  const source = useDecodedSource(url)
  onValue(source)
  return null
}

describe('useDecodedSource', () => {
  it('is null when url is undefined', () => {
    const values: Array<HTMLCanvasElement | null> = []
    render(<Probe url={undefined} onValue={(c) => values.push(c)} />)
    expect(values[values.length - 1]).toBeNull()
  })

  it('decodes the url into a canvas sized to the image', async () => {
    const drawn: string[] = []
    const fakeCtx = { drawImage: () => { drawn.push('drew') } }
    const realCreate = document.createElement.bind(document)
    vi.spyOn(document, 'createElement').mockImplementation((t: string) => {
      if (t === 'canvas') return { width: 0, height: 0, getContext: () => fakeCtx } as unknown as HTMLElement
      return realCreate(t)
    })
    class FakeImage { naturalWidth = 80; naturalHeight = 60; onload: (() => void) | null = null
      set src(_s: string) { this.onload?.() } }
    ;(globalThis as unknown as { Image: unknown }).Image = FakeImage as unknown

    const values: Array<HTMLCanvasElement | null> = []
    render(<Probe url="blob:x" onValue={(c) => values.push(c)} />)

    await waitFor(() => expect(values[values.length - 1]).not.toBeNull())
    const canvas = values[values.length - 1] as unknown as { width: number; height: number }
    expect(canvas.width).toBe(80)
    expect(canvas.height).toBe(60)
    expect(drawn).toContain('drew')
    vi.restoreAllMocks()
  })
})
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/overlay/annotations/useDecodedSource.test.ts`
Expected: FAIL — `useDecodedSource.ts` does not exist.

- [ ] **Step 3: Write the hook**

```typescript
// ABOUTME: Decodes a screenshot blob URL into an offscreen canvas used to sample
// ABOUTME: pixels for the live pixelated-redaction preview. Re-decodes per URL.
import { useEffect, useState } from 'react'

/**
 * Decode `url` into an offscreen canvas at the image's natural size. Returns null
 * until decoded and whenever `url` is undefined. The canvas is the sampling source
 * for live redaction pixelation; reading from it is untainted (blob URL is same-origin).
 */
export function useDecodedSource(url: string | undefined): HTMLCanvasElement | null {
  const [source, setSource] = useState<HTMLCanvasElement | null>(null)

  useEffect(() => {
    if (!url) {
      setSource(null)
      return
    }
    let cancelled = false
    const img = new Image()
    img.onload = () => {
      if (cancelled) return
      const canvas = document.createElement('canvas')
      canvas.width = img.naturalWidth
      canvas.height = img.naturalHeight
      const ctx = canvas.getContext('2d')
      if (!ctx) return
      ctx.drawImage(img, 0, 0)
      setSource(canvas)
    }
    img.src = url
    return () => {
      cancelled = true
    }
  }, [url])

  return source
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/overlay/annotations/useDecodedSource.test.ts`
Expected: PASS — null-when-undefined and decode-to-canvas cases green.

- [ ] **Step 5: Commit**

```bash
git add src/overlay/annotations/useDecodedSource.ts src/overlay/annotations/useDecodedSource.test.ts
git commit -m "feat: useDecodedSource hook for redaction pixel sampling"
```

---

## Task 4: Live pixelation in DrawingLayer

**Files:**
- Modify: `src/overlay/annotations/DrawingLayer.tsx` (imports, `Props`, signature, rect rendering, new component)
- Test: `src/overlay/annotations/DrawingLayer.test.tsx` (add a pixelated-background suite)

- [ ] **Step 1: Write the failing tests**

Append to `src/overlay/annotations/DrawingLayer.test.tsx`. The harness seeds one redaction rect and renders with a given `redactionStyle`; the assertions check the rect div's inline background. (jsdom has no canvas 2D context, so the live canvas can't paint here — drawing correctness is covered by Task 1/Task 2 and verified live in Task 6. These tests pin the *branching*: pixelated must not paint the mosaic.)

```typescript
import { nextRectId } from "./DrawingLayer";

function redactionState(style: "pixelated" | "branded" | "solid"): AnnotationsState {
  const id = nextRectId();
  return {
    rects: [{ id, x: 20, y: 20, w: 120, h: 80, note: "", kind: "redaction" }],
    history: [],
    active: null,
    lastCreatedId: id,
  };
}

function RedactionHarness({ style }: { style: "pixelated" | "branded" | "solid" }) {
  const [state, dispatch] = useReducer(reducer, redactionState(style));
  return (
    <DrawingLayer
      state={state}
      dispatch={dispatch}
      redactMode
      redactionStyle={style}
    />
  );
}

describe("DrawingLayer pixelated redaction preview", () => {
  it("does not paint the mosaic background for pixelated rects", () => {
    const { container } = render(<RedactionHarness style="pixelated" />);
    const rect = container.querySelector('[data-rect-id]') as HTMLDivElement;
    // No grey-checker mosaic and no 12px tiling — the live canvas owns the fill.
    expect(rect.style.background).not.toContain("svg");
    expect(rect.style.backgroundSize).toBe("");
  });

  it("still paints the mosaic background for branded rects", () => {
    const { container } = render(<RedactionHarness style="branded" />);
    const rect = container.querySelector('[data-rect-id]') as HTMLDivElement;
    expect(rect.style.background).toContain("svg");
    expect(rect.style.backgroundSize).toBe("12px 12px");
  });

  it("paints a solid fill for solid rects", () => {
    const { container } = render(<RedactionHarness style="solid" />);
    const rect = container.querySelector('[data-rect-id]') as HTMLDivElement;
    expect(rect.style.background).toContain("rgb(36, 31, 27)");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/overlay/annotations/DrawingLayer.test.tsx`
Expected: FAIL — pixelated currently paints the mosaic (`background` contains `svg`, `backgroundSize` is `12px 12px`), so the first test fails.

- [ ] **Step 3: Add the imports and props**

In `src/overlay/annotations/DrawingLayer.tsx`, extend the imports (after the existing `import type { RedactionStyle }` line):

```typescript
import { pixelateBlocks, redactionBlockPx } from "./pixelate";
import { useDecodedSource } from "./useDecodedSource";
import type { Rect } from "./types";
```

Add the two props to the `Props` type (after `redactionStyle?: RedactionStyle;`):

```typescript
  // Decoded screenshot blob URL + the CSS box it occupies (viewport in frozen
  // mode, the window rect in window mode). Enables the live pixelated preview.
  sourceImageUrl?: string;
  imageRect?: { x: number; y: number; w: number; h: number };
```

Update the component signature destructuring (was `{ state, dispatch, clipRect, onOutsideClick, redactMode = false, redactionStyle = 'branded' }`):

```typescript
export function DrawingLayer({ state, dispatch, clipRect, onOutsideClick, redactMode = false, redactionStyle = 'branded', sourceImageUrl, imageRect }: Props) {
```

- [ ] **Step 4: Decode the source inside the component**

Immediately inside `DrawingLayer`, before the existing `const ref = useRef…`, add:

```typescript
  const source = useDecodedSource(sourceImageUrl);
```

- [ ] **Step 5: Branch the rect background and render the canvas**

In the rect `.map(...)`, replace the `background`/`backgroundSize` lines in the rect div's `style` (currently `background: isRedaction ? redactionPreviewBg(redactionStyle) : RECT_FILL,` and the `backgroundSize` line) with:

```typescript
              background: isRedaction
                ? (redactionStyle === "pixelated" ? "transparent" : redactionPreviewBg(redactionStyle))
                : RECT_FILL,
              backgroundSize:
                isRedaction && redactionStyle !== "solid" && redactionStyle !== "pixelated"
                  ? "12px 12px"
                  : undefined,
```

Then, as the first child inside the existing `{r.w >= MIN_RECT_DIMENSION && r.h >= MIN_RECT_DIMENSION && ( <> ... )}` fragment — before the `{!isRedaction && (` badge block — add the live canvas:

```typescript
                {isRedaction && redactionStyle === "pixelated" && source && imageRect && (
                  <PixelatedRedactionFill source={source} imageRect={imageRect} rect={r} />
                )}
```

- [ ] **Step 6: Add the `PixelatedRedactionFill` component**

Add this component near the top of `DrawingLayer.tsx` (after `redactionPreviewBg`):

```typescript
// Renders the live pixelated cover for one redaction rect: samples the decoded
// screenshot under the rect and fills averaged blocks identical to the bake.
// Redraws are coalesced to one animation frame so fast drags don't pile up work.
function PixelatedRedactionFill({
  source,
  imageRect,
  rect,
}: {
  source: HTMLCanvasElement;
  imageRect: { x: number; y: number; w: number; h: number };
  rect: Rect;
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    const draw = () => {
      rafRef.current = null;
      const canvas = canvasRef.current;
      if (!canvas) return;
      const ctx = canvas.getContext("2d");
      const sctx = source.getContext("2d");
      if (!ctx || !sctx) return;

      // CSS→source scale from displayed geometry (== capture DPR), so frozen and
      // window modes are both correct without a separate scaleFactor.
      const scale = source.width / imageRect.w;
      const block = redactionBlockPx(scale);
      const sx = Math.round((rect.x - imageRect.x) * scale);
      const sy = Math.round((rect.y - imageRect.y) * scale);
      const sw = Math.round(rect.w * scale);
      const sh = Math.round(rect.h * scale);
      if (sw <= 0 || sh <= 0) return;

      // Backing store spans the whole rect in source px; out-of-image areas stay
      // transparent, matching the bake which covers only captured pixels.
      canvas.width = sw;
      canvas.height = sh;
      ctx.clearRect(0, 0, sw, sh);

      const cx = Math.max(0, sx);
      const cy = Math.max(0, sy);
      const cw = Math.min(source.width, sx + sw) - cx;
      const ch = Math.min(source.height, sy + sh) - cy;
      if (cw <= 0 || ch <= 0) return;

      const region = sctx.getImageData(cx, cy, cw, ch);
      const averaged = pixelateBlocks(region.data, cw, ch, block);
      ctx.putImageData(new ImageData(averaged, cw, ch), cx - sx, cy - sy);
    };

    if (rafRef.current == null) rafRef.current = requestAnimationFrame(draw);
    return () => {
      if (rafRef.current != null) cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    };
  }, [source, imageRect.x, imageRect.y, imageRect.w, imageRect.h, rect.x, rect.y, rect.w, rect.h]);

  return (
    <canvas
      ref={canvasRef}
      aria-hidden
      data-testid="redaction-pixel-canvas"
      style={{ position: "absolute", inset: 0, width: "100%", height: "100%", pointerEvents: "none" }}
    />
  );
}
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `npx vitest run src/overlay/annotations/DrawingLayer.test.tsx`
Expected: PASS — pixelated paints no mosaic; branded/solid unchanged; existing clipRect tests still green.

- [ ] **Step 8: Commit**

```bash
git add src/overlay/annotations/DrawingLayer.tsx src/overlay/annotations/DrawingLayer.test.tsx
git commit -m "feat: live pixelated redaction preview in DrawingLayer"
```

---

## Task 5: Wire the source + image rect from Overlay

**Files:**
- Modify: `src/overlay/Overlay.tsx` (both `DrawingLayer` call sites)

- [ ] **Step 1: Pass props at the window-mode call site**

In `src/overlay/Overlay.tsx`, the window-mode `<DrawingLayer …>` (inside `if (mode === 'window' && windowPayload)`, currently passing `clipRect={window_rect_css}` … `redactionStyle={redactionStyle}`) — add two props:

```typescript
        <DrawingLayer
          state={annotations}
          dispatch={dispatch}
          clipRect={window_rect_css}
          onOutsideClick={() => {
            if (annotations.rects.length === 0) {
              bridge.cancel()
            } else {
              setShowDiscardModal(true)
            }
          }}
          redactMode={redactMode}
          redactionStyle={redactionStyle}
          sourceImageUrl={imageUrl ?? undefined}
          imageRect={window_rect_css}
        />
```

- [ ] **Step 2: Pass props at the frozen/live call site**

The second `<DrawingLayer …>` (in the main return, currently `state` / `dispatch` / `redactMode` / `redactionStyle`) — add:

```typescript
      <DrawingLayer
        state={annotations}
        dispatch={dispatch}
        redactMode={redactMode}
        redactionStyle={redactionStyle}
        sourceImageUrl={imageUrl ?? undefined}
        imageRect={{ x: 0, y: 0, w: window.innerWidth, h: window.innerHeight }}
      />
```

- [ ] **Step 3: Type-check and run the full suite**

Run: `npx tsc --noEmit && npx vitest run`
Expected: PASS — no type errors; all overlay tests green.

- [ ] **Step 4: Commit**

```bash
git add src/overlay/Overlay.tsx
git commit -m "feat: feed screenshot source + rect to redaction preview"
```

---

## Task 6: Live verification in the app

**Files:** none (manual verification per the `run` skill).

- [ ] **Step 1: Launch the app**

Run: `PATH="$HOME/.cargo/bin:$PATH" npm run tauri:dev`
Expected: app appears in the macOS menu bar; no console errors.

- [ ] **Step 2: Verify the live preview matches the saved image**

1. Set the redaction style to **Pixelated** (toolbar or tray).
2. Trigger a capture, draw a redaction rect over text/content.
3. **Verify:** the rect interior shows a real coarse pixelation of the content underneath (not a grey checker), and it **re-samples as you move/resize** the rect.
4. Save, open the saved `{id}_clean.png`.
5. **Verify (pixel-level, not eyeball — grid drift is invisible to the eye):** the redacted region in the file is identical to the preview. Open the saved PNG and the live screenshot side by side and compare a block's exact color, or overlay them in an image tool at 100%; the block boundaries and per-block colors must coincide. A 1px horizontal/vertical shift of the block grid means the `scale`/offset mapping is off — investigate before signing off.
6. Draw a rect and confirm there is **no grey checker** at any point — pixelated never shows the mosaic.
7. Switch to **Branded** and **Solid**: confirm their previews are unchanged.
8. Test **window mode**: select a window, draw a pixelated redaction inside it, confirm the preview samples the window capture correctly and (per step 5) matches the saved file pixel-for-pixel. This is the path with the residual ±1px fractional-DPR risk — scrutinize block-edge alignment here.

- [ ] **Step 2b: Verify Retina parity**

On a Retina display, confirm block coarseness in the preview matches the saved file (the `scale = source.width / imageRect.w` mapping must equal the capture DPR).

- [ ] **Step 3: Final full-suite gate**

Run: `npx vitest run`
Expected: PASS — entire suite green. Report exact pass/fail counts.

---

## Self-review notes

- **Spec coverage:** scope (pixelated only, live re-sample) → Tasks 1/4/5; coordinate mapping & block parity → Task 1 (`redactionBlockPx`, anchored grid, truncating mean) + Task 4 (`scale` from geometry); shared helper / `annotate.ts` refactor → Tasks 1–2; transparent fallback → Task 4 Step 5 (`"transparent"` background, canvas only when `source && imageRect`); partial-out edge → Task 4 Step 6 clamp; testing → Tasks 1–4; security (no new escape, `pointer-events:none`, samples already-shown blob) → Task 4 component.
- **Rounding decision:** helper truncates (`Math.floor`) to match `redact.rs`; `annotate.ts` adopts it — preview equals the final saved image on both Tauri and Electron paths (Dmitry's stated priority).
- **Type consistency:** `pixelateBlocks(src, width, height, block)` and `redactionBlockPx(scaleFactor)` used identically in Tasks 1, 2, 4; `useDecodedSource(url) → HTMLCanvasElement | null` consumed in Task 4; `imageRect`/`sourceImageUrl` prop names identical in Tasks 4 and 5.
- **Test-scope caveats (do not over-trust the unit layer):**
  - The truncation-vs-round guard lives in **Task 1** (`pixelateBlocks` floor cases). Task 2's annotate integration test has a mean of exactly 2.0, so it does *not* distinguish floor from round — it only proves the region is read → averaged → written back. Don't treat it as the truncation pin.
  - jsdom has no Canvas 2D context, so `PixelatedRedactionFill` never paints in `DrawingLayer.test.tsx` (`source` stays null). Those tests pin only the *branching* (pixelated paints no mosaic). **Task 6 is therefore the only verification that the canvas actually renders the correct pixels — it is mandatory, not optional.**
