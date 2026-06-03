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
    measureText: vi.fn((s: string) => ({ width: s.length * 8 })),
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

  it('draws a caption box and the note text when a note is present', async () => {
    mockCtx.fillText.mockClear()
    mockCtx.roundRect.mockClear()
    const { renderAnnotated } = await import('./annotate')

    installImageMock()

    const annotations = [{ id: 'a1', x: 10, y: 20, w: 100, h: 50, note: 'fix this' }]
    await renderAnnotated('blob:fake', annotations, 1)

    // Caption text is drawn (a short note stays one line: "fix this").
    const texts = mockCtx.fillText.mock.calls.map((c) => c[0])
    expect(texts).toContain('fix this')
    // Pill badge (1 roundRect) + caption border + caption fill (2 roundRect) = 3.
    expect(mockCtx.roundRect).toHaveBeenCalledTimes(3)
  })

  it('draws no caption when the note is blank', async () => {
    mockCtx.fillText.mockClear()
    mockCtx.roundRect.mockClear()
    const { renderAnnotated } = await import('./annotate')

    installImageMock()

    const annotations = [{ id: 'a1', x: 10, y: 20, w: 100, h: 50, note: '   ' }]
    await renderAnnotated('blob:fake', annotations, 1)

    const texts = mockCtx.fillText.mock.calls.map((c) => c[0])
    expect(texts).toEqual(['01']) // only the badge label, no caption text
    expect(mockCtx.roundRect).toHaveBeenCalledTimes(1) // only the pill badge
  })
})
