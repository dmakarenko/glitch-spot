// ABOUTME: Tests for useDecodedSource — decodes a blob URL into an offscreen
// ABOUTME: sampling canvas, once per URL, null until ready and when url is undefined.
// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest'
import { render, cleanup, waitFor, act } from '@testing-library/react'
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

  it('clears the stale canvas immediately when the url changes, until the new image decodes', () => {
    const pending: Array<{ onload: (() => void) | null }> = []
    const realCreate = document.createElement.bind(document)
    vi.spyOn(document, 'createElement').mockImplementation((t: string) => {
      if (t === 'canvas') return { width: 0, height: 0, getContext: () => ({ drawImage: () => {} }) } as unknown as HTMLElement
      return realCreate(t)
    })
    // Does NOT auto-fire onload — the test drives decode completion explicitly.
    class FakeImage { naturalWidth = 40; naturalHeight = 40; onload: (() => void) | null = null
      set src(_s: string) { pending.push(this) } }
    ;(globalThis as unknown as { Image: unknown }).Image = FakeImage as unknown

    const values: Array<HTMLCanvasElement | null> = []
    const { rerender } = render(<Probe url="blob:a" onValue={(c) => values.push(c)} />)
    act(() => { pending[0].onload?.() })
    const canvasA = values[values.length - 1]
    expect(canvasA).not.toBeNull() // A decoded

    // Switch to a new blob: the stale canvas must be dropped synchronously, so
    // EVERY render under the new URL (including the first, before B decodes)
    // returns null — a redaction drawn now can never sample the old frame.
    values.length = 0
    act(() => { rerender(<Probe url="blob:b" onValue={(c) => values.push(c)} />) })
    expect(values).not.toContain(canvasA) // stale A never shown under blob:b
    expect(values[values.length - 1]).toBeNull()

    act(() => { pending[1].onload?.() })
    expect(values[values.length - 1]).not.toBeNull() // B decoded
    vi.restoreAllMocks()
  })
})
