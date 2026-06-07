// ABOUTME: Main-process entry point for @glitch-spot/electron.
// ABOUTME: GlitchSpot.init() creates the overlay window and wires IPC + hotkey.
import path from 'node:path'
import { randomUUID } from 'node:crypto'
import {
  BrowserWindow,
  globalShortcut,
  ipcMain,
  screen,
  type App,
} from 'electron'
import type { GlitchSnapshot, GlitchSpotConfig, GlitchSpotInstance, AnnotationPayload } from './types'

// Mirrors src/overlay/annotations/label.ts. Inlined to keep this package self-contained.
function formatLabel(idx: number): string {
  return String(idx + 1).padStart(2, '0')
}

export class GlitchSpot {
  private overlay: BrowserWindow
  private mainWindow: BrowserWindow
  private config: Required<GlitchSpotConfig>
  private app: App
  private active = false

  private snapshotHandler!: (event: Electron.IpcMainEvent, payload: unknown) => void
  private cancelHandler!: () => void
  private triggerHandler!: () => void
  private syncBoundsHandler!: () => void

  private constructor(app: App, mainWindow: BrowserWindow, config: Required<GlitchSpotConfig>) {
    this.app = app
    this.mainWindow = mainWindow
    this.config = config

    this.overlay = new BrowserWindow({
      parent: mainWindow,
      transparent: true,
      frame: false,
      alwaysOnTop: true,
      skipTaskbar: true,
      show: false,
      webPreferences: {
        preload: path.join(__dirname, 'overlay-preload.cjs'),
        contextIsolation: true,
        nodeIntegration: false,
      },
    })

    this.overlay.loadFile(path.join(__dirname, '..', 'overlay', 'overlay.html'))

    this.setupIpc()
    this.trackMainWindow()
  }

  private setupIpc(): void {
    this.triggerHandler = () => { void this.trigger() }

    this.snapshotHandler = async (event: Electron.IpcMainEvent, rawPayload: unknown) => {
      if (event.sender !== this.overlay.webContents) {
        console.warn('[glitch-spot] Rejected snapshot IPC from unexpected sender')
        return
      }

      const payload = rawPayload as {
        annotations: AnnotationPayload[]
        cleanBase64: string
        annotatedBase64: string
      }
      this.overlay.hide()
      this.active = false

      const bounds = this.mainWindow.getContentBounds()
      const display = screen.getDisplayMatching(this.mainWindow.getBounds())

      const snapshot: GlitchSnapshot = {
        id: `snap_${randomUUID().replace(/-/g, '').slice(0, 12)}`,
        created_at: new Date().toISOString(),
        mode: 'frozen',
        annotations: payload.annotations
          .filter((a) => a.kind !== 'redaction')
          .map((a, idx) => ({
            id: a.id,
            shape: 'rect',
            coords: {
              x: a.x * display.scaleFactor,
              y: a.y * display.scaleFactor,
              w: a.w * display.scaleFactor,
              h: a.h * display.scaleFactor,
            },
            note: a.note,
            label: formatLabel(idx),
          })),
        context: {
          screen: {
            width: bounds.width * display.scaleFactor,
            height: bounds.height * display.scaleFactor,
            scale_factor: display.scaleFactor,
          },
          os: {
            platform: process.platform,
            version: (process as NodeJS.Process & { getSystemVersion?: () => string }).getSystemVersion?.() ?? '',
          },
          app: {
            name: this.app.getName(),
            version: this.app.getVersion(),
            metadata: this.config.metadata(),
          },
        },
        images: {
          clean_base64: payload.cleanBase64,
          annotated_base64: payload.annotatedBase64,
        },
      }

      try {
        await this.config.onSnapshot(snapshot)
      } catch (err) {
        console.error('[glitch-spot] onSnapshot error:', err)
      }
    }

    this.cancelHandler = () => {
      this.overlay.hide()
      this.active = false
    }

    ipcMain.on('glitch-spot:trigger', this.triggerHandler)
    ipcMain.on('glitch-spot:snapshot', this.snapshotHandler)
    ipcMain.on('glitch-spot:cancel', this.cancelHandler)
  }

  private trackMainWindow(): void {
    this.syncBoundsHandler = () => {
      if (this.active || this.overlay.isDestroyed()) return
      const b = this.mainWindow.getContentBounds()
      this.overlay.setBounds(b)
    }
    this.mainWindow.on('resize', this.syncBoundsHandler)
    this.mainWindow.on('move', this.syncBoundsHandler)
  }

  async trigger(): Promise<void> {
    if (this.active) return
    this.active = true

    try {
      const nativeImage = await this.mainWindow.webContents.capturePage()

      // Guard: stop() may have been called while we were capturing
      if (this.overlay.isDestroyed()) {
        this.active = false
        return
      }

      const base64Png = nativeImage.toPNG().toString('base64')
      const bounds = this.mainWindow.getContentBounds()
      this.overlay.setBounds(bounds)
      this.overlay.show()
      this.overlay.focus()
      this.overlay.webContents.send('glitch-spot:frame', base64Png)
    } catch (err) {
      console.error('[glitch-spot] trigger error:', err)
      this.active = false
    }
  }

  static init(app: App, mainWindow: BrowserWindow, config: GlitchSpotConfig): GlitchSpotInstance {
    if (!config.onSnapshot) throw new Error('[glitch-spot] onSnapshot is required')

    const fullConfig: Required<GlitchSpotConfig> = {
      hotkey: config.hotkey ?? '',
      onSnapshot: config.onSnapshot,
      metadata: config.metadata ?? (() => ({})),
    }

    const instance = new GlitchSpot(app, mainWindow, fullConfig)

    if (fullConfig.hotkey) {
      const ok = globalShortcut.register(fullConfig.hotkey, () => { void instance.trigger() })
      if (!ok) console.warn(`[glitch-spot] Could not register hotkey: ${fullConfig.hotkey}`)
    }

    return {
      open() { void instance.trigger() },
      stop() {
        if (fullConfig.hotkey) globalShortcut.unregister(fullConfig.hotkey)
        ipcMain.off('glitch-spot:trigger', instance.triggerHandler)
        ipcMain.off('glitch-spot:snapshot', instance.snapshotHandler)
        ipcMain.off('glitch-spot:cancel', instance.cancelHandler)
        instance.mainWindow.off('resize', instance.syncBoundsHandler)
        instance.mainWindow.off('move', instance.syncBoundsHandler)
        if (!instance.overlay.isDestroyed()) instance.overlay.destroy()
      },
    }
  }
}
