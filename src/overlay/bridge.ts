// ABOUTME: Runtime-detected IPC bridge for the overlay UI.
// ABOUTME: TauriBridge wraps @tauri-apps/api; ElectronBridge wraps window.glitchSpotBridge.
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import type { AnnotationPayload, GlitchSpotNativeBridge } from '../../packages/electron/src/types'

export interface OverlayBridge {
  onFrame(cb: (blobUrl: string) => void): Promise<() => void>
  getFrame(): Promise<string | null>
  saveSnapshot(mode: string, annotations: AnnotationPayload[]): Promise<void>
  cancel(): void
  // Ask the native layer to redraw the OS cursor. The crosshair is a CSS
  // cursor, and macOS only re-evaluates the displayed cursor on a mouse event;
  // after the overlay is shown under a stationary pointer the cursor stays an
  // arrow until the user moves the mouse.
  refreshCursor(): void
  // Signal that the latest present has painted. The window-mode overlay is
  // shown at alpha 0 (rendering but invisible) so it repaints the target
  // display's layout before becoming visible; this tells the native layer the
  // frame has landed and it can fade the window in to alpha 1. No-op natively
  // when no reveal is pending.
  revealOverlay(): void
}

type TauriFrame = {
  width: number; height: number; scale_factor: number; png: number[]; display_index: number
}

function frameToBlob(frame: TauriFrame): string {
  const bytes = new Uint8Array(frame.png)
  const blob = new Blob([bytes], { type: 'image/png' })
  return URL.createObjectURL(blob)
}

class TauriBridge implements OverlayBridge {
  async onFrame(cb: (blobUrl: string) => void): Promise<() => void> {
    return listen<TauriFrame>('glitch-spot://frame', (e) => cb(frameToBlob(e.payload)))
  }
  async getFrame(): Promise<string | null> {
    const frame = await invoke<TauriFrame | null>('get_latest_frame').catch(() => null)
    return frame ? frameToBlob(frame) : null
  }
  async saveSnapshot(mode: string, annotations: AnnotationPayload[]): Promise<void> {
    await invoke('save_snapshot', { mode, annotations })
  }
  cancel(): void {
    invoke('hide_overlay')
  }
  refreshCursor(): void {
    invoke('refresh_cursor')
  }
  revealOverlay(): void {
    invoke('reveal_window')
  }
}

declare global {
  interface Window {
    __TAURI__?: unknown
    glitchSpotBridge?: GlitchSpotNativeBridge
  }
}

class ElectronBridge implements OverlayBridge {
  private cleanBase64: string | null = null
  private cleanBlobUrl: string | null = null
  private frameHandler: ((b64: string) => void) | null = null

  async onFrame(cb: (blobUrl: string) => void): Promise<() => void> {
    this.frameHandler = (b64: string) => {
      if (this.cleanBlobUrl) URL.revokeObjectURL(this.cleanBlobUrl)
      this.cleanBase64 = b64
      const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
      const blob = new Blob([bytes], { type: 'image/png' })
      this.cleanBlobUrl = URL.createObjectURL(blob)
      cb(this.cleanBlobUrl)
    }
    window.glitchSpotBridge!.onFrame(this.frameHandler)
    return () => {
      if (this.frameHandler) window.glitchSpotBridge!.offFrame(this.frameHandler)
    }
  }

  async getFrame(): Promise<string | null> {
    return null
  }

  async saveSnapshot(_mode: string, annotations: AnnotationPayload[]): Promise<void> {
    if (!this.cleanBase64 || !this.cleanBlobUrl) {
      throw new Error('[glitch-spot] saveSnapshot called before first frame')
    }
    const { renderAnnotated } = await import('./annotate')
    const annotatedBase64 = await renderAnnotated(
      this.cleanBlobUrl,
      annotations,
      window.devicePixelRatio,
    )
    // mode is always 'frozen' in v1 — not passed through IPC, set by main process
    window.glitchSpotBridge!.saveSnapshot(annotations, this.cleanBase64, annotatedBase64)
  }

  cancel(): void {
    window.glitchSpotBridge!.cancel()
  }
  // Electron renders the overlay in a real focused window; the CSS-cursor
  // redraw quirk is specific to the Tauri menu-bar overlay.
  refreshCursor(): void {}
  // Electron shows the overlay window directly; the alpha-0 reveal dance is
  // specific to the Tauri single-window overlay repositioned across displays.
  revealOverlay(): void {}
}

export function createBridge(): OverlayBridge {
  if (window.glitchSpotBridge) return new ElectronBridge()
  return new TauriBridge()
}
