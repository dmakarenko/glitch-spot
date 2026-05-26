// ABOUTME: Preload script for the Glitch Spot overlay BrowserWindow.
// ABOUTME: Exposes window.glitchSpotBridge for the overlay React UI via contextBridge.
import { contextBridge, ipcRenderer } from 'electron'
import type { GlitchSpotNativeBridge, AnnotationPayload } from './types'

type FrameHandler = (base64Png: string) => void
const frameHandlers = new Set<FrameHandler>()

ipcRenderer.on('glitch-spot:frame', (_event, base64Png: string) => {
  if (typeof base64Png !== 'string' || base64Png.length === 0) return
  frameHandlers.forEach((cb) => cb(base64Png))
})

const bridge: GlitchSpotNativeBridge = {
  onFrame(cb: FrameHandler): void {
    frameHandlers.add(cb)
  },
  offFrame(cb: FrameHandler): void {
    frameHandlers.delete(cb)
  },
  saveSnapshot(
    mode: string,
    annotations: AnnotationPayload[],
    cleanBase64: string,
    annotatedBase64: string,
  ): void {
    ipcRenderer.send('glitch-spot:snapshot', { mode, annotations, cleanBase64, annotatedBase64 })
  },
  cancel(): void {
    ipcRenderer.send('glitch-spot:cancel')
  },
}

contextBridge.exposeInMainWorld('glitchSpotBridge', bridge)
