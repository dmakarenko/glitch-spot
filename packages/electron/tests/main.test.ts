// ABOUTME: Unit tests for the GlitchSpot main-process SDK class.
// ABOUTME: Tests config validation and IPC wiring using Electron mocks.
import { describe, it, expect, vi, beforeEach } from 'vitest'

const mockGlobalShortcut = { register: vi.fn().mockReturnValue(true), unregister: vi.fn() }
const mockIpcMain = { on: vi.fn(), removeAllListeners: vi.fn() }

const makeWindow = () => ({
  getBounds: vi.fn().mockReturnValue({ x: 0, y: 0, width: 1440, height: 900 }),
  getContentBounds: vi.fn().mockReturnValue({ x: 0, y: 0, width: 1440, height: 900 }),
  webContents: {
    capturePage: vi.fn().mockResolvedValue({ toPNG: () => Buffer.from('fake-png') }),
    send: vi.fn(),
    on: vi.fn(),
  },
  on: vi.fn(),
  loadFile: vi.fn().mockResolvedValue(undefined),
  show: vi.fn(),
  hide: vi.fn(),
  focus: vi.fn(),
  destroy: vi.fn(),
  isDestroyed: vi.fn().mockReturnValue(false),
  setBounds: vi.fn(),
})

let BrowserWindowConstructor: ReturnType<typeof vi.fn>

vi.mock('electron', () => {
  BrowserWindowConstructor = vi.fn().mockImplementation(function () { return makeWindow() })
  return {
    globalShortcut: mockGlobalShortcut,
    ipcMain: mockIpcMain,
    BrowserWindow: BrowserWindowConstructor,
    screen: {
      getDisplayMatching: vi.fn().mockReturnValue({ scaleFactor: 2 }),
    },
    app: { getName: vi.fn().mockReturnValue('TestApp'), getVersion: vi.fn().mockReturnValue('1.0.0') },
  }
})

beforeEach(() => {
  vi.clearAllMocks()
  vi.resetModules()
})

describe('GlitchSpot.init', () => {
  it('throws if onSnapshot is not provided', async () => {
    const { GlitchSpot } = await import('../src/main')
    const fakeApp = {} as Electron.App
    const fakeWindow = makeWindow() as unknown as Electron.BrowserWindow
    expect(() =>
      GlitchSpot.init(fakeApp, fakeWindow, {} as never),
    ).toThrow('onSnapshot is required')
  })

  it('registers a global shortcut when hotkey is provided', async () => {
    const { GlitchSpot } = await import('../src/main')
    const fakeApp = { getName: vi.fn().mockReturnValue('TestApp'), getVersion: vi.fn().mockReturnValue('1.0.0') } as unknown as Electron.App
    const fakeWindow = makeWindow() as unknown as Electron.BrowserWindow
    GlitchSpot.init(fakeApp, fakeWindow, {
      hotkey: 'Ctrl+Shift+G',
      onSnapshot: vi.fn(),
    })
    expect(mockGlobalShortcut.register).toHaveBeenCalledWith('Ctrl+Shift+G', expect.any(Function))
  })

  it('does not register a shortcut when hotkey is omitted', async () => {
    const { GlitchSpot } = await import('../src/main')
    const fakeApp = { getName: vi.fn().mockReturnValue('TestApp'), getVersion: vi.fn().mockReturnValue('1.0.0') } as unknown as Electron.App
    const fakeWindow = makeWindow() as unknown as Electron.BrowserWindow
    GlitchSpot.init(fakeApp, fakeWindow, { onSnapshot: vi.fn() })
    expect(mockGlobalShortcut.register).not.toHaveBeenCalled()
  })

  it('returns an instance with open() and stop()', async () => {
    const { GlitchSpot } = await import('../src/main')
    const fakeApp = { getName: vi.fn().mockReturnValue('TestApp'), getVersion: vi.fn().mockReturnValue('1.0.0') } as unknown as Electron.App
    const fakeWindow = makeWindow() as unknown as Electron.BrowserWindow
    const instance = GlitchSpot.init(fakeApp, fakeWindow, { onSnapshot: vi.fn() })
    expect(typeof instance.open).toBe('function')
    expect(typeof instance.stop).toBe('function')
  })
})
