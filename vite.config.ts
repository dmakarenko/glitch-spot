// ABOUTME: Vite build configuration for Glitch Spot's overlay + picker frontends.
// ABOUTME: Two HTML entrypoints — overlay.html and picker.html — each bundled separately.
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    rollupOptions: {
      input: {
        overlay: resolve(__dirname, "overlay.html"),
        picker: resolve(__dirname, "picker.html"),
      },
    },
  },
});
