// ABOUTME: Main-process entry point for @glitch-spot/electron.
// ABOUTME: GlitchSpot.init() creates the overlay window and wires IPC + hotkey.
import path from 'node:path'
import {
  BrowserWindow,
  globalShortcut,
  ipcMain,
  screen,
  type App,
} from 'electron'
import type { GlitchSnapshot, GlitchSpotConfig, GlitchSpotInstance, AnnotationPayload } from './types'

export class GlitchSpot {
  private overlay: BrowserWindow
  private mainWindow: BrowserWindow
  private config: Required<GlitchSpotConfig>
  private app: App
  private active = false

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

    this.overlay.loadFile(path.join(__dirname, '..', 'overlay', 'index.html'))

    this.setupIpc()
    this.trackMainWindow()
  }

  private setupIpc(): void {
    ipcMain.on('glitch-spot:trigger', () => this.trigger())

    ipcMain.on('glitch-spot:snapshot', async (_event, payload: {
      mode: string
      annotations: AnnotationPayload[]
      cleanBase64: string
      annotatedBase64: string
    }) => {
      this.overlay.hide()
      this.active = false

      const bounds = this.mainWindow.getContentBounds()
      const display = screen.getDisplayMatching(this.mainWindow.getBounds())

      const snapshot: GlitchSnapshot = {
        id: `snap_${Date.now().toString(16).padStart(12, '0')}`,
        created_at: new Date().toISOString(),
        mode: 'frozen',
        annotations: payload.annotations.map((a) => ({
          id: a.id,
          shape: 'rect',
          coords: { x: a.x, y: a.y, w: a.w, h: a.h },
          note: a.note,
        })),
        context: {
          screen: {
            width: bounds.width,
            height: bounds.height,
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
    })

    ipcMain.on('glitch-spot:cancel', () => {
      this.overlay.hide()
      this.active = false
    })
  }

  private trackMainWindow(): void {
    const syncBounds = () => {
      if (this.active || this.overlay.isDestroyed()) return
      const b = this.mainWindow.getContentBounds()
      this.overlay.setBounds(b)
    }
    this.mainWindow.on('resize', syncBounds)
    this.mainWindow.on('move', syncBounds)
  }

  async trigger(): Promise<void> {
    if (this.active) return
    this.active = true

    const nativeImage = await this.mainWindow.webContents.capturePage()
    const base64Png = nativeImage.toPNG().toString('base64')

    const bounds = this.mainWindow.getContentBounds()
    this.overlay.setBounds(bounds)
    this.overlay.show()
    this.overlay.focus()
    this.overlay.webContents.send('glitch-spot:frame', base64Png)
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
      const ok = globalShortcut.register(fullConfig.hotkey, () => instance.trigger())
      if (!ok) console.warn(`[glitch-spot] Could not register hotkey: ${fullConfig.hotkey}`)
    }

    return {
      open() { instance.trigger() },
      stop() {
        if (fullConfig.hotkey) globalShortcut.unregister(fullConfig.hotkey)
        ipcMain.removeAllListeners('glitch-spot:snapshot')
        ipcMain.removeAllListeners('glitch-spot:cancel')
        ipcMain.removeAllListeners('glitch-spot:trigger')
        if (!instance.overlay.isDestroyed()) instance.overlay.destroy()
      },
    }
  }
}
