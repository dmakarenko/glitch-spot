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
