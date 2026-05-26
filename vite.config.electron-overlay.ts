// ABOUTME: Vite config for building the overlay UI into the Electron SDK package.
// ABOUTME: Same source as the Tauri overlay, output goes to packages/electron/overlay/.
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()],
  base: './',  // required for file:// loading via BrowserWindow.loadFile()
  build: {
    outDir: resolve(__dirname, 'packages/electron/overlay'),
    emptyOutDir: true,
    rollupOptions: {
      input: {
        overlay: resolve(__dirname, 'overlay.html'),  // use 'overlay' key to match Tauri config
      },
    },
  },
})
