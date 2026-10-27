// ABOUTME: Vitest config for @glitch-spot/electron — jsdom environment for React component tests.
// ABOUTME: Canvas mock required for annotated-PNG renderer tests.
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    setupFiles: [],
    globals: true,
  },
})
