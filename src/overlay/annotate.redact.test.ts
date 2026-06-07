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
