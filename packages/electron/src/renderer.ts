// ABOUTME: Renderer-safe exports for @glitch-spot/electron/renderer.
// ABOUTME: No Node.js APIs — safe to import in any Electron renderer process.
export { GlitchSpotButton } from './GlitchSpotButton'

declare global {
  interface Window {
    glitchSpot?: { open(): void }
  }
}

export const glitchSpot = {
  open(): void {
    window.glitchSpot?.open()
  },
}
