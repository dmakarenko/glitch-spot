// ABOUTME: Canvas-based annotated PNG renderer for the overlay UI.
// ABOUTME: Draws annotation overlays on a screenshot and returns a base64 PNG.
import type { AnnotationPayload } from '../../packages/electron/src/types'

export async function renderAnnotated(
  _blobUrl: string,
  _annotations: AnnotationPayload[],
  _devicePixelRatio: number,
): Promise<string> {
  throw new Error('renderAnnotated: not yet implemented — Task 4 will provide the real implementation')
}
