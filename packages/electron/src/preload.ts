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
