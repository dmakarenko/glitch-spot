// ABOUTME: Vite build configuration for Glitch Spot's single-page overlay frontend.
// ABOUTME: Only the overlay HTML entry point is declared; the main window has been removed.
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

// Default port matches tauri.conf.json devUrl. Override with VITE_PORT=<n> to run a
// second instance in parallel, e.g. from another worktree:
//   VITE_PORT=1421 npm run tauri dev -- --config '{"build":{"devUrl":"http://localhost:1421"}}'
const port = Number(process.env.VITE_PORT ?? 1420);

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port, strictPort: true },
  build: {
    rollupOptions: {
      input: {
        overlay: resolve(__dirname, "overlay.html"),
      },
    },
  },
});
