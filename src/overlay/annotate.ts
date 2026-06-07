// ABOUTME: Canvas-based annotation renderer used by the Electron overlay path.
// ABOUTME: Draws annotation rectangles, pill badges, and note captions onto the clean image.
import type { AnnotationPayload } from '../../packages/electron/src/types'
import type { RedactionStyle } from './bridge'
import { formatLabel } from './annotations/label'
import { wrapNote } from './annotations/wrapNote'

const COVER_DARK = '#0c0c0c' // DESIGN.md background — matches redact.rs COVER_DARK
const ACCENT = '#ee6633'     // matches redact.rs ACCENT
const ACCENT_DIM = '#78341a' // matches redact.rs ACCENT_DIM
const BLOCK_CSS = 12         // coarse block; matches redact.rs BLOCK_CSS

// Cover redaction regions on `ctx`. Mirrors src-tauri/src/snapshot/redact.rs so
// the Tauri and Electron outputs match. Operates in physical pixels.
function coverRegions(
  ctx: CanvasRenderingContext2D,
  regions: AnnotationPayload[],
  scaleFactor: number,
  style: RedactionStyle,
) {
  const block = Math.max(2, Math.round(BLOCK_CSS * scaleFactor))
  for (const r of regions) {
    const x = Math.round(r.x * scaleFactor)
    const y = Math.round(r.y * scaleFactor)
    const w = Math.round(r.w * scaleFactor)
    const h = Math.round(r.h * scaleFactor)
    if (w <= 0 || h <= 0) continue

    if (style === 'solid') {
      ctx.fillStyle = COVER_DARK
      ctx.fillRect(x, y, w, h)
      continue
    }

    if (style === 'branded') {
      let row = 0
      for (let by = y; by < y + h; by += block, row++) {
        let col = 0
        for (let bx = x; bx < x + w; bx += block, col++) {
          const bw = Math.min(block, x + w - bx)
          const bh = Math.min(block, y + h - by)
          const pick = (row + col * 3) % 4
          ctx.fillStyle = pick === 0 ? ACCENT : pick === 1 ? ACCENT_DIM : COVER_DARK
          ctx.fillRect(bx, by, bw, bh)
        }
      }
      continue
    }

    // pixelated: average each block from the source and refill it.
    for (let by = y; by < y + h; by += block) {
      for (let bx = x; bx < x + w; bx += block) {
        const bw = Math.min(block, x + w - bx)
        const bh = Math.min(block, y + h - by)
        const data = ctx.getImageData(bx, by, bw, bh).data
        let rr = 0, gg = 0, bb = 0, aa = 0
        const n = data.length / 4
        for (let i = 0; i < data.length; i += 4) {
          rr += data[i]; gg += data[i + 1]; bb += data[i + 2]; aa += data[i + 3]
        }
        ctx.fillStyle = `rgba(${Math.round(rr / n)}, ${Math.round(gg / n)}, ${Math.round(bb / n)}, ${(aa / n / 255).toFixed(3)})`
        ctx.fillRect(bx, by, bw, bh)
      }
    }
  }
}

/**
 * Draws the clean image and covers its redaction regions, returning a raw base64
 * PNG (no data: prefix). The annotated image must be rendered FROM this output,
 * so the covered pixels never survive in either image.
 */
export async function redactClean(
  cleanBlobUrl: string,
  annotations: AnnotationPayload[],
  scaleFactor: number,
  style: RedactionStyle,
): Promise<string> {
  const img = await loadImage(cleanBlobUrl)
  const canvas = document.createElement('canvas')
  canvas.width = img.naturalWidth
  canvas.height = img.naturalHeight
  const ctx = canvas.getContext('2d')!
  ctx.drawImage(img, 0, 0)
  const regions = annotations.filter((a) => a.kind === 'redaction')
  coverRegions(ctx, regions, scaleFactor, style)
  return canvas.toDataURL('image/png').split(',')[1]
}

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
    if (rect.kind === 'redaction') return
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
    if (rect.kind === 'redaction') return
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
