// ABOUTME: Shared TypeScript types for the @glitch-spot/electron SDK.
// ABOUTME: GlitchSnapshot is wire-compatible with the standalone app's schema.

export interface GlitchSnapshot {
  id: string
  created_at: string
  mode: 'frozen'
  annotations: Annotation[]
  context: {
    screen: { width: number; height: number; scale_factor: number }
    os: { platform: string; version: string }
    app: {
      name: string
      version: string
      metadata?: Record<string, unknown>
    }
  }
  images: {
    clean_base64: string
    annotated_base64: string
  }
}

export interface Annotation {
  id: string
  shape: 'rect'
  coords: { x: number; y: number; w: number; h: number }
  note: string
  label: string
}

export interface AnnotationPayload {
  id: string
  x: number
  y: number
  w: number
  h: number
  note: string
}

export interface GlitchSpotConfig {
  hotkey?: string
  onSnapshot: (snapshot: GlitchSnapshot) => void | Promise<void>
  metadata?: () => Record<string, unknown>
}

export interface GlitchSpotInstance {
  open(): void
  stop(): void
}

export interface GlitchSpotNativeBridge {
  onFrame(cb: (base64Png: string) => void): void
  offFrame(cb: (base64Png: string) => void): void
  saveSnapshot(annotations: AnnotationPayload[], cleanBase64: string, annotatedBase64: string): void
  cancel(): void
}
