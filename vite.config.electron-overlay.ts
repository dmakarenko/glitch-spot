// ABOUTME: Vite config for building the overlay UI into the Electron SDK package.
// ABOUTME: Same source as the Tauri overlay, output goes to packages/electron/overlay/.
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: resolve(__dirname, 'packages/electron/overlay'),
    emptyOutDir: true,
    rollupOptions: {
      input: {
        index: resolve(__dirname, 'overlay.html'),
      },
    },
  },
})
