// ABOUTME: Tests for the shared redaction block-averaging helper.
// ABOUTME: Pins the truncating mean, edge clamping, and block-size formula to redact.rs.
import { describe, it, expect } from 'vitest'
import { pixelateBlocks, redactionBlockPx, REDACTION_BLOCK_CSS, forEachBlockMean } from './pixelate'

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

describe('forEachBlockMean', () => {
  it('visits each block with its bounds and truncated mean', () => {
    // width 3, block 2 → block A=cols[0,1] bounds (0,0,2,1), block B=col[2] bounds (2,0,1,1).
    // Row 60, 60, 241 → A mean 60, B mean 241.
    const visits: Array<[number, number, number, number, number]> = []
    forEachBlockMean(
      buffer(3, 1, (x) => (x < 2 ? [60, 60, 60, 255] : [241, 241, 241, 255])),
      3, 1, 2,
      (bx, by, bw, bh, r) => visits.push([bx, by, bw, bh, r]),
    )
    expect(visits).toEqual([
      [0, 0, 2, 1, 60],
      [2, 0, 1, 1, 241],
    ])
  })

  it('truncates the mean (matches redact.rs)', () => {
    // 0, 0, 2 → 0.666… → 0.
    let mean = -1
    forEachBlockMean(buffer(3, 1, (x) => (x === 2 ? [2, 2, 2, 255] : [0, 0, 0, 255])), 3, 1, 12, (_x, _y, _w, _h, r) => { mean = r })
    expect(mean).toBe(0)
  })

  it('averages a sub-rectangle of a wider buffer given its stride and offset', () => {
    // 5x4 source with a distinct value per pixel; region is 3x2 at (1,1).
    const full = buffer(5, 4, (x, y) => [x * 10 + y, 0, 0, 255])
    const cropped = buffer(3, 2, (x, y) => [(x + 1) * 10 + (y + 1), 0, 0, 255])
    const fromCrop: number[][] = []
    const fromFull: number[][] = []
    forEachBlockMean(cropped, 3, 2, 2, (...v) => fromCrop.push(v))
    forEachBlockMean(full, 3, 2, 2, (...v) => fromFull.push(v), 5, 1 * 5 + 1)
    expect(fromFull).toEqual(fromCrop)
  })
})
