// ABOUTME: Tests for the redactClean and renderAnnotated functions in annotate.ts.
// ABOUTME: Verifies redaction covering and annotation label numbering correctness.
import { describe, it, expect, beforeAll, afterEach } from 'vitest'
import { redactClean } from './annotate'

// The pristine document.createElement, captured before any test monkey-patches
// it, so each test restores a clean baseline and they don't wrap one another.
let pristineCreateElement: typeof document.createElement

beforeAll(() => {
  ;(globalThis as unknown as { devicePixelRatio: number }).devicePixelRatio = 1
  pristineCreateElement = document.createElement.bind(document)
})

afterEach(() => {
  ;(document as unknown as { createElement: typeof document.createElement }).createElement =
    pristineCreateElement
})

describe('renderAnnotated', () => {
  it('numbers annotations only, skipping redactions', async () => {
    const texts: string[] = []
    const fakeCtx = {
      drawImage: () => {},
      strokeRect: () => {},
      fillRect: () => {},
      beginPath: () => {}, roundRect: () => {}, fill: () => {}, stroke: () => {},
      measureText: () => ({ width: 10 }),
      fillText: (t: string) => texts.push(t),
      getImageData: () => ({ data: new Uint8ClampedArray(4) }),
      putImageData: () => {},
      set fillStyle(_v: string) {}, get fillStyle() { return '#000' },
      set strokeStyle(_v: string) {}, get strokeStyle() { return '#000' },
      set lineWidth(_v: number) {}, get lineWidth() { return 1 },
      set font(_v: string) {}, get font() { return '' },
      set textAlign(_v: string) {}, get textAlign() { return 'left' },
      set textBaseline(_v: string) {}, get textBaseline() { return 'alphabetic' },
    }
    const fakeCanvas = { width: 200, height: 200, getContext: () => fakeCtx, toDataURL: () => 'data:image/png;base64,AAAA' }
    ;(document as unknown as { createElement: (t: string) => unknown }).createElement = (t: string) => t === 'canvas' ? fakeCanvas : pristineCreateElement(t)
    class FakeImage { naturalWidth = 200; naturalHeight = 200; onload: (() => void) | null = null; set src(_s: string) { this.onload?.() } }
    ;(globalThis as unknown as { Image: unknown }).Image = FakeImage as unknown

    const { renderAnnotated } = await import('./annotate')
    await renderAnnotated('blob:x', [
      { id: 'a1', x: 5,  y: 5, w: 30, h: 30, note: 'first',  kind: 'annotation' },
      { id: 'r1', x: 40, y: 5, w: 30, h: 30, note: '',       kind: 'redaction' },
      { id: 'a2', x: 80, y: 5, w: 30, h: 30, note: 'second', kind: 'annotation' },
    ], 1)
    // Only the two annotation badges were drawn, numbered 01 and 02 (redaction skipped, not consuming 02).
    const labels = texts.filter((t) => /^\d{2}$/.test(t))
    expect(labels).toEqual(['01', '02'])
  })
})

describe('redactClean', () => {
  it('returns a base64 string and covers a solid region', async () => {
    const fills: Array<[number, number, number, number]> = []
    const fakeCtx = {
      drawImage: () => {},
      set fillStyle(_v: string) {},
      get fillStyle() { return '#000' },
      fillRect: (x: number, y: number, w: number, h: number) => fills.push([x, y, w, h]),
      beginPath: () => {}, roundRect: () => {}, fill: () => {},
      getImageData: () => ({ data: new Uint8ClampedArray(4) }),
      putImageData: () => {},
    }
    const fakeCanvas = {
      width: 0, height: 0,
      getContext: () => fakeCtx,
      toDataURL: () => 'data:image/png;base64,AAAA',
    }
    ;(document as unknown as { createElement: (t: string) => unknown }).createElement = (t: string) =>
      t === 'canvas' ? fakeCanvas : pristineCreateElement(t)
    class FakeImage { naturalWidth = 40; naturalHeight = 40; onload: (() => void) | null = null; set src(_s: string) { this.onload?.() } }
    ;(globalThis as unknown as { Image: unknown }).Image = FakeImage as unknown

    const out = await redactClean('blob:x', [
      { id: 'r', x: 5, y: 5, w: 10, h: 10, note: '', kind: 'redaction' },
    ], 1, 'solid')

    expect(typeof out).toBe('string')
    expect(out.length).toBeGreaterThan(0)
    expect(fills.length).toBeGreaterThan(0)
  })

  it('pixelated writes the truncated block mean back to the region', async () => {
    // 4x4 source image, scale 1 → one 12px block covers the whole 4x4 region.
    const W = 4, H = 4
    const store = new Uint8ClampedArray(W * H * 4)
    // Half black, half (5,5,5,255): 8 px of 0 + 8 px of 5 → mean 2.5 → floor 2 (round would give 3).
    for (let i = 0; i < store.length; i += 4) {
      const px = i / 4
      const v = px < 8 ? 0 : 5
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

  it('branded covers the region with only palette colors, per block', async () => {
    const fills: string[] = []
    let current = '#000'
    const fakeCtx = {
      drawImage: () => {},
      set fillStyle(v: string) { current = v },
      get fillStyle() { return current },
      fillRect: () => { fills.push(current) },
      beginPath: () => {}, roundRect: () => {}, fill: () => {},
      getImageData: () => ({ data: new Uint8ClampedArray(4) }),
      putImageData: () => {},
    }
    const fakeCanvas = { width: 0, height: 0, getContext: () => fakeCtx, toDataURL: () => 'data:image/png;base64,AAAA' }
    ;(document as unknown as { createElement: (t: string) => unknown }).createElement = (t: string) =>
      t === 'canvas' ? fakeCanvas : pristineCreateElement(t)
    class FakeImage { naturalWidth = 60; naturalHeight = 60; onload: (() => void) | null = null; set src(_s: string) { this.onload?.() } }
    ;(globalThis as unknown as { Image: unknown }).Image = FakeImage as unknown

    await redactClean('blob:x', [{ id: 'r', x: 0, y: 0, w: 48, h: 48, note: '', kind: 'redaction' }], 1, 'branded')
    expect(fills.length).toBeGreaterThan(1)
    for (const c of fills) expect(['#0c0c0c', '#ee6633', '#78341a']).toContain(c)
  })
})
