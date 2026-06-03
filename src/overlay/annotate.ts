// ABOUTME: Canvas-based annotation renderer used by the Electron overlay path.
// ABOUTME: Draws annotation rectangles, pill badges, and note captions onto the clean image.
import type { AnnotationPayload } from '../../packages/electron/src/types'
import { formatLabel } from './annotations/label'
import { wrapNote } from './annotations/wrapNote'

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = reject
    img.src = url
  })
}

/**
 * Renders annotation rectangles, pill-shaped labels, and note captions onto cleanBlobUrl.
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

  // Pass 1: rectangles + pill badges.
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

  // Pass 2: note captions, drawn on top of all rectangles.
  const CAPTION_MAX_W = 240 * scaleFactor
  const CAPTION_OFFSET = 6 * scaleFactor
  const CAPTION_PAD_X = 8 * scaleFactor
  const CAPTION_PAD_Y = 6 * scaleFactor
  const CAPTION_RADIUS = 6 * scaleFactor
  const CAPTION_BORDER_W = Math.max(1, Math.round(1 * scaleFactor))
  const CAPTION_FONT_PX = 13 * scaleFactor
  const CAPTION_LINE_HEIGHT = 1.4
  const lineH = Math.round(CAPTION_FONT_PX * CAPTION_LINE_HEIGHT)
  // Cap the caption width to the canvas so a narrow capture wraps the note to fit
  // rather than overflowing the right edge and clipping the text.
  const captionMaxW = Math.min(CAPTION_MAX_W, canvas.width)

  annotations.forEach((rect) => {
    const note = rect.note.trim()
    if (!note) return

    const x = rect.x * scaleFactor
    const y = rect.y * scaleFactor
    const h = rect.h * scaleFactor

    ctx.font = `${CAPTION_FONT_PX}px system-ui`
    const measure = (s: string) => ctx.measureText(s).width
    const textMaxW = Math.max(1, captionMaxW - 2 * CAPTION_PAD_X)
    const lines = wrapNote(note, textMaxW, measure)

    const widest = lines.reduce((m, l) => Math.max(m, measure(l)), 0)
    const boxW = Math.min(captionMaxW, Math.ceil(widest) + 2 * CAPTION_PAD_X)
    const boxH = lines.length * lineH + 2 * CAPTION_PAD_Y

    // Below the rect; flip above on bottom-overflow; clamp fully on-canvas.
    const belowY = y + h + CAPTION_OFFSET
    let boxY = belowY + boxH > canvas.height ? y - CAPTION_OFFSET - boxH : belowY
    boxY = Math.max(0, Math.min(boxY, canvas.height - boxH))
    const boxX = Math.max(0, Math.min(x, canvas.width - boxW))

    // Border layer, then inset background fill → clean 1px rounded border.
    ctx.fillStyle = '#ee6633'
    ctx.beginPath()
    ctx.roundRect(boxX, boxY, boxW, boxH, CAPTION_RADIUS)
    ctx.fill()
    ctx.fillStyle = '#161616'
    ctx.beginPath()
    ctx.roundRect(
      boxX + CAPTION_BORDER_W,
      boxY + CAPTION_BORDER_W,
      boxW - 2 * CAPTION_BORDER_W,
      boxH - 2 * CAPTION_BORDER_W,
      Math.max(0, CAPTION_RADIUS - CAPTION_BORDER_W),
    )
    ctx.fill()

    ctx.fillStyle = '#aaaaaa'
    ctx.textAlign = 'left'
    ctx.textBaseline = 'top'
    lines.forEach((line, i) => {
      ctx.fillText(line, boxX + CAPTION_PAD_X, boxY + CAPTION_PAD_Y + i * lineH)
    })
    ctx.textBaseline = 'alphabetic'
  })

  return canvas.toDataURL('image/png').split(',')[1]
}
