// ABOUTME: Decodes a screenshot blob URL into an offscreen canvas used to sample
// ABOUTME: pixels for the live pixelated-redaction preview. Re-decodes per URL.
import { useEffect, useState } from 'react'

/**
 * Decode `url` into an offscreen canvas at the image's natural size. Returns null
 * until decoded and whenever `url` is undefined. The canvas is the sampling source
 * for live redaction pixelation; reading from it is untainted (blob URL is same-origin).
 */
export function useDecodedSource(url: string | undefined): HTMLCanvasElement | null {
  const [decoded, setDecoded] = useState<{ url: string; canvas: HTMLCanvasElement } | null>(null)

  useEffect(() => {
    if (!url) {
      setDecoded(null)
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
      setDecoded({ url, canvas })
    }
    img.src = url
    return () => {
      cancelled = true
    }
  }, [url])

  // Tie the canvas to the URL it was decoded from and gate on a match. On a
  // blob→blob swap the stale canvas is dropped synchronously in the first render
  // under the new URL — no one-frame window where a redaction samples the old
  // frame (a passive-effect clear would land a frame late).
  return decoded && decoded.url === url ? decoded.canvas : null
}
