// ABOUTME: Unit tests for the overlay bridge factory.
// ABOUTME: Verifies that createBridge() returns the correct implementation.
import { describe, it, expect, vi, beforeEach } from 'vitest'

beforeEach(() => {
  delete window.__TAURI__
  delete window.glitchSpotBridge
})

describe('createBridge', () => {
  it('returns TauriBridge when neither __TAURI__ nor glitchSpotBridge is set (Tauri 2 default)', async () => {
    // Tauri 2 does not set window.__TAURI__ unless withGlobalTauri is enabled
    const { createBridge } = await import('./bridge')
    const bridge = createBridge()
    expect(bridge.constructor.name).toBe('TauriBridge')
  })

  it('returns TauriBridge when __TAURI__ is present (Tauri 1 / withGlobalTauri)', async () => {
    window.__TAURI__ = {}
    const { createBridge } = await import('./bridge')
    const bridge = createBridge()
    expect(bridge.constructor.name).toBe('TauriBridge')
  })

  it('returns ElectronBridge when glitchSpotBridge is present', async () => {
    window.glitchSpotBridge = {
      onFrame: vi.fn(),
      offFrame: vi.fn(),
      saveSnapshot: vi.fn(),
      cancel: vi.fn(),
    }
    const { createBridge } = await import('./bridge')
    const bridge = createBridge()
    expect(bridge.constructor.name).toBe('ElectronBridge')
  })
})
