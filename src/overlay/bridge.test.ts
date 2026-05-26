// ABOUTME: Unit tests for the overlay bridge factory.
// ABOUTME: Verifies that createBridge() returns the correct implementation.
import { describe, it, expect, vi, beforeEach } from 'vitest'

beforeEach(() => {
  // @ts-expect-error test setup
  delete window.__TAURI__
  // @ts-expect-error test setup
  delete window.glitchSpotBridge
})

describe('createBridge', () => {
  it('returns TauriBridge when __TAURI__ is present', async () => {
    // @ts-expect-error test setup
    window.__TAURI__ = {}
    const { createBridge } = await import('./bridge')
    const bridge = createBridge()
    expect(bridge.constructor.name).toBe('TauriBridge')
  })

  it('returns ElectronBridge when glitchSpotBridge is present', async () => {
    // @ts-expect-error test setup
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
