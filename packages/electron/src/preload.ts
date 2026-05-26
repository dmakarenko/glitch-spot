// ABOUTME: Optional preload script for the host app's main BrowserWindow.
// ABOUTME: Exposes window.glitchSpot.open() in the renderer via contextBridge.
import { contextBridge, ipcRenderer } from 'electron'

export function glitchSpotPreload(): void {
  contextBridge.exposeInMainWorld('glitchSpot', {
    open(): void {
      ipcRenderer.send('glitch-spot:trigger')
    },
  })
}

export const preloadPath = __filename

// Self-execute when loaded directly as an Electron preload script.
// process.type is 'renderer' in any preload/renderer context.
if (process.type === 'renderer') {
  glitchSpotPreload()
}
