// ABOUTME: Tests for the canvas-based annotated PNG renderer.
// ABOUTME: Verifies that renderAnnotated returns a base64 string without errors.
import { describe, it, expect, vi, beforeAll } from 'vitest'

// Minimal canvas mock — jsdom does not implement canvas rendering.
beforeAll(() => {
  HTMLCanvasElement.prototype.getContext = vi.fn().mockReturnValue({
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
  })
  HTMLCanvasElement.prototype.toDataURL = vi.fn().mockReturnValue('data:image/png;base64,mock123')
})

describe('renderAnnotated', () => {
  it('returns a base64 string for a single annotation', async () => {
    const { renderAnnotated } = await import('./annotate')

    const fakeUrl = 'blob:fake'

    // Intercept the Image constructor and immediately fire onload after src is set.
    // Must use a class so it can be called with `new`.
    vi.spyOn(window, 'Image' as never).mockImplementationOnce(function (this: HTMLImageElement) {
      Object.defineProperty(this, 'naturalWidth', { value: 800, configurable: true })
      Object.defineProperty(this, 'naturalHeight', { value: 600, configurable: true })

      // Defining src as a setter so assigning img.src triggers onload asynchronously.
      let _src = ''
      // eslint-disable-next-line @typescript-eslint/no-this-alias
      const self = this
      Object.defineProperty(this, 'src', {
        set(url: string) {
          _src = url
          queueMicrotask(() => self.onload?.(new Event('load')))
        },
        get() {
          return _src
        },
        configurable: true,
      })
    } as never)

    const annotations = [{ id: 'a1', x: 10, y: 20, w: 100, h: 50, note: 'test note' }]
    await expect(renderAnnotated(fakeUrl, annotations, 2)).resolves.toBe('mock123')
  })
})
