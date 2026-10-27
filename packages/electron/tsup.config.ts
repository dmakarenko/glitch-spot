// ABOUTME: tsup build config for @glitch-spot/electron.
// ABOUTME: Dual ESM+CJS for main/renderer; CJS-only for Electron preload scripts.
import { defineConfig } from 'tsup'

export default defineConfig([
  {
    entry: { main: 'src/main.ts', renderer: 'src/renderer.ts' },
    format: ['esm', 'cjs'],
    dts: true,
    external: ['electron', 'react', 'react-dom'],
    sourcemap: true,
    outExtension({ format }) {
      return { js: format === 'cjs' ? '.cjs' : '.mjs' }
    },
  },
  {
    entry: { preload: 'src/preload.ts', 'overlay-preload': 'src/overlay-preload.ts' },
    format: ['cjs'],
    dts: false,
    external: ['electron'],
    sourcemap: true,
    outExtension() {
      return { js: '.cjs' }
    },
  },
])
