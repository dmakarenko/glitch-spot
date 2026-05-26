// ABOUTME: Canvas-based annotation renderer used by the Electron overlay path.
// ABOUTME: Draws annotation rectangles and badges onto the clean captured image.
import type { AnnotationPayload } from '../../packages/electron/src/types'

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = reject
    img.src = url
  })
}

/**
 * Renders annotation rectangles and numbered badges onto cleanBlobUrl.
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

  annotations.forEach((rect, idx) => {
    const x = rect.x * scaleFactor
    const y = rect.y * scaleFactor
    const w = rect.w * scaleFactor
    const h = rect.h * scaleFactor

    ctx.strokeStyle = '#f97316'
    ctx.lineWidth = 2 * scaleFactor
    ctx.strokeRect(x, y, w, h)

    const badgeR = 10 * scaleFactor
    const badgeCx = x + badgeR
    const badgeCy = y + badgeR
    ctx.fillStyle = '#f97316'
    ctx.beginPath()
    ctx.arc(badgeCx, badgeCy, badgeR, 0, Math.PI * 2)
    ctx.fill()

    ctx.fillStyle = '#ffffff'
    ctx.font = `bold ${11 * scaleFactor}px sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText(String(idx + 1), badgeCx, badgeCy)
    ctx.textAlign = 'left'
    ctx.textBaseline = 'alphabetic'
  })

  return canvas.toDataURL('image/png').split(',')[1]
}
