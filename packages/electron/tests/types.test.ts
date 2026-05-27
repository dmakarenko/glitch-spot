// ABOUTME: Type-level and runtime smoke tests for the GlitchSnapshot schema.
// ABOUTME: Verifies field names and shapes match the spec before any implementation.
import { describe, it, expect } from 'vitest'
import type { GlitchSnapshot, GlitchSpotConfig, AnnotationPayload } from '../src/types'

describe('GlitchSnapshot shape', () => {
  it('accepts a fully-populated snapshot', () => {
    const snap: GlitchSnapshot = {
      id: 'snap_abc123456789',
      created_at: '2026-05-26T10:00:00.000Z',
      mode: 'frozen',
      annotations: [
        { id: 'a1', shape: 'rect', coords: { x: 10, y: 20, w: 100, h: 50 }, note: 'broken button', label: '01' },
      ],
      context: {
        screen: { width: 1440, height: 900, scale_factor: 2 },
        os: { platform: 'darwin', version: '14.5' },
        app: { name: 'MyApp', version: '1.2.3', metadata: { userId: 42 } },
      },
      images: {
        clean_base64: 'abc123',
        annotated_base64: 'def456',
      },
    }
    expect(snap.id).toMatch(/^snap_/)
    expect(snap.mode).toBe('frozen')
    expect(snap.annotations).toHaveLength(1)
    expect(snap.images.clean_base64).toBe('abc123')
    expect(snap.images.annotated_base64).toBe('def456')
    expect(snap.context.app.name).toBe('MyApp')
  })

  it('GlitchSpotConfig requires onSnapshot', () => {
    const config: GlitchSpotConfig = {
      onSnapshot: async (_snap) => {},
    }
    expect(typeof config.onSnapshot).toBe('function')
  })
})
