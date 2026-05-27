# Region Labels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> Source seed: [`docs/seeds/2026-05-27-region-labels-seed.md`](../seeds/2026-05-27-region-labels-seed.md).

**Goal:** Show an auto-derived numeric label (`01`, `02`, `03`, …) on every annotation region, both on the live overlay during drawing and on the exported annotated PNG, using a shared formatter so the two surfaces cannot diverge.

**Architecture:** Add a tiny pure `formatLabel(idx)` helper. The live overlay (`DrawingLayer.tsx`) renders a small filled-orange pill `<div>` at the top-left corner of each rectangle, gated by the same `MIN_RECT_DIMENSION` check as the delete-X. The canvas exporter (`annotate.ts`) draws the same pill geometry on the captured image, switching from the current filled circle. Both surfaces consume the same `formatLabel`. No IPC schema change.

**Tech Stack:** React 19 + TypeScript (Vite), Vitest, HTML Canvas 2D (`roundRect` + `fill`).

---

## File structure

```
src/overlay/annotations/
  label.ts             # NEW — formatLabel(idx) helper
  label.test.ts        # NEW — unit tests for formatLabel
  DrawingLayer.tsx     # MODIFY — render badge div at top-left of each rect
src/overlay/
  annotate.ts          # MODIFY — pill badge + formatLabel(idx)
  annotate.test.ts     # MODIFY — assert "01"/"02", roundRect call
```

No other files change. `packages/electron/src/types.ts` `AnnotationPayload` stays as-is.

---

## Shared constants

Both surfaces use the same pill geometry:

- **Pill CSS width:** 26 px
- **Pill CSS height:** 18 px
- **Pill corner radius:** 9 px
- **Pill fill:** `#ee6633`
- **Pill text:** white, bold 11 px sans-serif, centered

These constants live next to their usage (one set in `DrawingLayer.tsx`, one set scaled by `scaleFactor` in `annotate.ts`). They do not need to be shared at the module level — they are visual constants that happen to match.

---

## Task 1: Add `formatLabel` helper with tests

**Files:**
- Create: `src/overlay/annotations/label.ts`
- Create: `src/overlay/annotations/label.test.ts`

- [ ] **Step 1: Write the failing test**

Create `src/overlay/annotations/label.test.ts` with this exact content:

```ts
// ABOUTME: Unit tests for the formatLabel helper used by the live overlay and the
// ABOUTME: canvas exporter to derive a region's visible label from its array index.
import { describe, it, expect } from 'vitest';
import { formatLabel } from './label';

describe('formatLabel', () => {
  it('zero-pads single-digit positions to two characters', () => {
    expect(formatLabel(0)).toBe('01');
    expect(formatLabel(8)).toBe('09');
  });

  it('returns two characters for the tenth and ninety-ninth positions', () => {
    expect(formatLabel(9)).toBe('10');
    expect(formatLabel(98)).toBe('99');
  });

  it('expands to three characters at one hundred and beyond, never truncating', () => {
    expect(formatLabel(99)).toBe('100');
    expect(formatLabel(998)).toBe('999');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- src/overlay/annotations/label.test.ts`
Expected: FAIL with a module-not-found error for `./label`.

- [ ] **Step 3: Write minimal implementation**

Create `src/overlay/annotations/label.ts` with this exact content:

```ts
// ABOUTME: Derives a region's visible label from its array index. Zero-pads to two
// ABOUTME: characters minimum, expands further for 100+ regions, never truncates.

export function formatLabel(idx: number): string {
  return String(idx + 1).padStart(2, '0');
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm test -- src/overlay/annotations/label.test.ts`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add src/overlay/annotations/label.ts src/overlay/annotations/label.test.ts
git commit -m "feat(overlay): add formatLabel helper for region labels"
```

---

## Task 2: Switch canvas exporter to pill badge using `formatLabel`

**Files:**
- Modify: `src/overlay/annotate.ts:32-57`
- Modify: `src/overlay/annotate.test.ts:1-69`

The current renderer draws a filled circle (`ctx.arc`) and labels it with `String(idx + 1)`. Replace with a `roundRect` pill of the shared geometry and the new formatter. The canvas API `roundRect` is supported by Chromium (Electron) and all modern browsers; for the jsdom-backed test we extend the mock.

- [ ] **Step 1: Update the test mock to include `roundRect` and assert pill draws**

Open `src/overlay/annotate.test.ts`. Replace the entire file contents with:

```ts
// ABOUTME: Tests for the canvas-based annotated PNG renderer.
// ABOUTME: Verifies that renderAnnotated returns a base64 string and draws
// ABOUTME: zero-padded labels on a pill badge for each annotation.
import { describe, it, expect, vi, beforeAll, afterAll } from 'vitest'

// Capture mock ctx as a module-level variable so assertions can reference it.
let mockCtx: ReturnType<typeof createMockCtx>

function createMockCtx() {
  return {
    drawImage: vi.fn(),
    strokeRect: vi.fn(),
    fillRect: vi.fn(),
    fillText: vi.fn(),
    measureText: vi.fn().mockReturnValue({ width: 50 }),
    beginPath: vi.fn(),
    arc: vi.fn(),
    roundRect: vi.fn(),
    fill: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    set strokeStyle(_v: string) {},
    set fillStyle(_v: string) {},
    set lineWidth(_v: number) {},
    set font(_v: string) {},
    set textAlign(_v: string) {},
    set textBaseline(_v: string) {},
  }
}

// Minimal canvas mock — jsdom does not implement canvas rendering.
beforeAll(() => {
  mockCtx = createMockCtx()
  HTMLCanvasElement.prototype.getContext = vi.fn().mockReturnValue(mockCtx)
  HTMLCanvasElement.prototype.toDataURL = vi.fn().mockReturnValue('data:image/png;base64,mock123')
})

afterAll(() => {
  vi.unstubAllGlobals()
})

function installImageMock() {
  const ImageMock = vi.fn().mockImplementation(function (this: HTMLImageElement) {
    Object.defineProperty(this, 'naturalWidth', { value: 800, configurable: true })
    Object.defineProperty(this, 'naturalHeight', { value: 600, configurable: true })
    Object.defineProperty(this, 'src', {
      set(_url: string) {
        queueMicrotask(() => {
          if (this.onload) this.onload(new Event('load'))
        })
      },
    })
  }) as unknown as typeof Image
  vi.stubGlobal('Image', ImageMock)
}

describe('renderAnnotated', () => {
  it('returns a base64 string for a single annotation and draws a pill badge with the label "01"', async () => {
    const { renderAnnotated } = await import('./annotate')

    installImageMock()

    const annotations = [{ id: 'a1', x: 10, y: 20, w: 100, h: 50, note: 'test note' }]
    const resultPromise = renderAnnotated('blob:fake', annotations, 2)
    await expect(resultPromise).resolves.toBe('mock123')

    expect(mockCtx.drawImage).toHaveBeenCalled()
    expect(mockCtx.strokeRect).toHaveBeenCalledWith(20, 40, 200, 100) // x*2, y*2, w*2, h*2
    expect(mockCtx.roundRect).toHaveBeenCalled() // pill badge drawn (not arc)
    expect(mockCtx.arc).not.toHaveBeenCalled() // no longer a circle
    expect(mockCtx.fillText).toHaveBeenCalledWith('01', expect.any(Number), expect.any(Number))
  })

  it('labels the tenth annotation "10"', async () => {
    mockCtx.fillText.mockClear()
    const { renderAnnotated } = await import('./annotate')

    installImageMock()

    const annotations = Array.from({ length: 10 }, (_, i) => ({
      id: `a${i}`,
      x: 10,
      y: 20,
      w: 100,
      h: 50,
      note: '',
    }))

    await renderAnnotated('blob:fake', annotations, 1)

    const labels = mockCtx.fillText.mock.calls.map((c) => c[0])
    expect(labels).toEqual(['01', '02', '03', '04', '05', '06', '07', '08', '09', '10'])
  })
})
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm test -- src/overlay/annotate.test.ts`
Expected: FAIL. Both tests fail because `annotate.ts` still calls `ctx.arc(...)` and `ctx.fillText(String(idx + 1), ...)`.

- [ ] **Step 3: Update the renderer**

Open `src/overlay/annotate.ts`. Replace the entire file contents with:

```ts
// ABOUTME: Canvas-based annotation renderer used by the Electron overlay path.
// ABOUTME: Draws annotation rectangles and pill badges onto the clean captured image.
import type { AnnotationPayload } from '../../packages/electron/src/types'
import { formatLabel } from './annotations/label'

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = reject
    img.src = url
  })
}

/**
 * Renders annotation rectangles and pill-shaped labels onto cleanBlobUrl.
 * Returns the result as a raw base64 PNG string (no data: prefix).
 * scaleFactor converts CSS coords to physical pixels (window.devicePixelRatio).
 */
export async function renderAnnotated(
  cleanBlobUrl: string,
  annotations: AnnotationPayload[],
  scaleFactor: number,
): Promise<string> {
  const img = await loadImage(cleanBlobUrl)
  const canvas = document.createElement('canvas')
  canvas.width = img.naturalWidth
  canvas.height = img.naturalHeight
  const ctx = canvas.getContext('2d')!

  ctx.drawImage(img, 0, 0)

  const PILL_W = 26 * scaleFactor
  const PILL_H = 18 * scaleFactor
  const PILL_R = 9 * scaleFactor

  annotations.forEach((rect, idx) => {
    const x = rect.x * scaleFactor
    const y = rect.y * scaleFactor
    const w = rect.w * scaleFactor
    const h = rect.h * scaleFactor

    ctx.strokeStyle = '#EE6633'
    ctx.lineWidth = 2 * scaleFactor
    ctx.strokeRect(x, y, w, h)

    // Pill badge straddling the rect's top-left corner (half outside).
    const pillX = x - PILL_W / 2
    const pillY = y - PILL_H / 2
    ctx.fillStyle = '#EE6633'
    ctx.beginPath()
    ctx.roundRect(pillX, pillY, PILL_W, PILL_H, PILL_R)
    ctx.fill()

    ctx.fillStyle = '#ffffff'
    ctx.font = `bold ${11 * scaleFactor}px sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText(formatLabel(idx), pillX + PILL_W / 2, pillY + PILL_H / 2)
    ctx.textAlign = 'left'
    ctx.textBaseline = 'alphabetic'
  })

  return canvas.toDataURL('image/png').split(',')[1]
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm test -- src/overlay/annotate.test.ts`
Expected: PASS, 2 tests.

- [ ] **Step 5: Run the full test suite to verify no regressions**

Run: `npm test`
Expected: PASS — all existing tests stay green, plus the new `label.test.ts` and the updated `annotate.test.ts` suites.

- [ ] **Step 6: Commit**

```bash
git add src/overlay/annotate.ts src/overlay/annotate.test.ts
git commit -m "feat(overlay): switch export badge to pill and zero-padded labels"
```

---

## Task 3: Render the badge on the live overlay

**Files:**
- Modify: `src/overlay/annotations/DrawingLayer.tsx:87-136`

There is no existing component-render test for `DrawingLayer.tsx`, and adding one purely for the badge would be brittle (it would lock DOM details). The shared `formatLabel` unit test (Task 1) plus the canvas test (Task 2) guarantee both surfaces format identically. This task is therefore visual-verification only.

- [ ] **Step 1: Add the badge to the rect wrapper**

Open `src/overlay/annotations/DrawingLayer.tsx`. At the top of the file, alongside the existing imports, add:

```ts
import { formatLabel } from './label';
```

Then locate the `state.rects.map((r) => (` block that renders each rect wrapper (currently lines 87-136). Inside that wrapper `<div>` — alongside the existing delete `<button>` — add a badge `<div>` immediately before the delete button. The full updated `state.rects.map` block should read:

```tsx
{state.rects.map((r, idx) => (
  <div
    key={r.id}
    data-rect-id={r.id}
    style={{
      position: "absolute",
      left: r.x,
      top: r.y,
      width: r.w,
      height: r.h,
      border: RECT_BORDER,
      background: RECT_FILL,
      boxSizing: "border-box",
      pointerEvents: "none",
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
      </>
    )}
  </div>
))}
```

Notes on the changes:
- `(r) =>` becomes `(r, idx) =>` to receive the array index for `formatLabel`.
- The delete button and the new badge are both gated by the existing `MIN_RECT_DIMENSION` check, so they appear together and never for tiny/in-progress rects. They are grouped under a single fragment (`<>…</>`).
- The badge `<div>` uses `aria-hidden` because the label is presentational; screen-reader users get the same information from the note textarea.
- Position math: pill is 26×18; offset `left: -13` centers it horizontally on the rect's left edge, `top: -9` centers it vertically on the rect's top edge — i.e., the pill straddles the top-left corner, matching the canvas exporter.
- `pointerEvents: "none"` on the badge so clicks pass through to the drawing surface (the rect wrapper itself is already `pointerEvents: "none"`; this keeps the badge consistent).

- [ ] **Step 2: Run the full test suite to verify no regressions**

Run: `npm test`
Expected: PASS — all suites still green.

- [ ] **Step 3: Visual smoke test**

The project supports two host environments for the same overlay code; either path verifies the badge.

**Option A — Tauri dev (standalone app):**

```bash
npm run tauri dev
```

Once the menu-bar tray icon appears, trigger the overlay (default hotkey is in `src-tauri/src/lib.rs`; commonly `Cmd+Shift+G`).

**Option B — Electron smoke test (SDK embed):**

```bash
cd packages/electron
npm run build:all
npx electron smoke-test/index.js
```

Then press `Ctrl+Shift+G` (per `smoke-test/index.js`) in the resulting window.

In either environment, draw three rectangles and verify:

1. Each rectangle shows an orange `01` / `02` / `03` pill straddling its top-left corner.
2. The badge does not appear until the rectangle reaches `MIN_RECT_DIMENSION` during the drag.
3. Deleting the middle rectangle renumbers the remaining rectangles to `01` / `02`.
4. Saving the snapshot produces an annotated PNG whose pill badges match the live view (Tauri: file written to the configured snapshots dir; Electron: `console.log` confirms the snapshot fired and `annotated_base64` is non-empty — load it into an image viewer to confirm).

If anything looks off, stop and inspect — do not adjust constants without comparing the live and exported renders side-by-side.

- [ ] **Step 4: Commit**

```bash
git add src/overlay/annotations/DrawingLayer.tsx
git commit -m "feat(overlay): render numbered pill badge on each annotation rect"
```

---

## Self-review checklist (reviewer applies before declaring done)

- [ ] `formatLabel` is imported and used by both `DrawingLayer.tsx` and `annotate.ts` (single source of truth for the format).
- [ ] No changes to `packages/electron/src/types.ts` (no IPC schema bump).
- [ ] No changes to `src/overlay/annotations/state.ts` or `state.test.ts` (no reducer changes).
- [ ] All three commits exist on the branch in order.
- [ ] `npm test` is green.
- [ ] The badge and the delete-X are both gated by the same `MIN_RECT_DIMENSION` check.
- [ ] The badge has no chromatic-aberration split (no `-2px / +2px` box-shadow split on the badge itself — only a soft drop shadow).
