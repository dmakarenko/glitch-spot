// ABOUTME: Tests for the overlay BrowserWindow preload script.
// ABOUTME: Verifies that the bridge API is exposed with the expected shape.
import { describe, it, expect, vi, beforeEach } from 'vitest'

const mockContextBridge = { exposeInMainWorld: vi.fn() }
const mockIpcRenderer = {
  on: vi.fn(),
  removeListener: vi.fn(),
  send: vi.fn(),
}

vi.mock('electron', () => ({
  contextBridge: mockContextBridge,
  ipcRenderer: mockIpcRenderer,
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.resetModules()
})

describe('overlay-preload', () => {
  it('exposes glitchSpotBridge with the required methods', async () => {
    await import('../src/overlay-preload')
    expect(mockContextBridge.exposeInMainWorld).toHaveBeenCalledWith(
      'glitchSpotBridge',
      expect.objectContaining({
        onFrame: expect.any(Function),
        offFrame: expect.any(Function),
        saveSnapshot: expect.any(Function),
        cancel: expect.any(Function),
      }),
    )
  })

  it('onFrame registers an ipcRenderer listener', async () => {
    await import('../src/overlay-preload')
    const bridge = mockContextBridge.exposeInMainWorld.mock.calls[0][1]
    const cb = vi.fn()
    bridge.onFrame(cb)
    expect(mockIpcRenderer.on).toHaveBeenCalledWith('glitch-spot:frame', expect.any(Function))
  })

  it('cancel sends glitch-spot:cancel IPC', async () => {
    await import('../src/overlay-preload')
    const bridge = mockContextBridge.exposeInMainWorld.mock.calls[0][1]
    bridge.cancel()
    expect(mockIpcRenderer.send).toHaveBeenCalledWith('glitch-spot:cancel')
  })
})
