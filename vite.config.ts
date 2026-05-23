// ABOUTME: Vite build configuration for Glitch Spot's single-page overlay frontend.
// ABOUTME: Only the overlay HTML entry point is declared; the main window has been removed.
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
      },
    },
  },
});
