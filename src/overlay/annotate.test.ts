// ABOUTME: Tests for the canvas-based annotated PNG renderer.
// ABOUTME: Verifies that renderAnnotated returns a base64 string without errors.
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

describe('renderAnnotated', () => {
  it('returns a base64 string for a single annotation', async () => {
    const { renderAnnotated } = await import('./annotate')

    const fakeUrl = 'blob:fake'

    const ImageMock = vi.fn().mockImplementationOnce(function (this: HTMLImageElement) {
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

    const annotations = [{ id: 'a1', x: 10, y: 20, w: 100, h: 50, note: 'test note' }]
    const resultPromise = renderAnnotated(fakeUrl, annotations, 2)
    await expect(resultPromise).resolves.toBe('mock123')

    expect(mockCtx.drawImage).toHaveBeenCalled()
    expect(mockCtx.strokeRect).toHaveBeenCalledWith(20, 40, 200, 100) // x*2, y*2, w*2, h*2
    expect(mockCtx.arc).toHaveBeenCalled() // badge circle drawn
    expect(mockCtx.fillText).toHaveBeenCalledWith('1', expect.any(Number), expect.any(Number))
  })
})
