import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port, and uses this to communicate with the frontend
// during development. If the port is changed, Tauri's devUrl must also be
// updated in src-tauri/tauri.conf.json accordingly.
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  // Tauri recommends disabling clearScreen so that Rust build output
  // is visible alongside the Vite dev server output.
  clearScreen: false,

  server: {
    // Tauri expects a fixed port; if it's in use, Vite should error
    // instead of picking the next available port.
    port: 1420,
    strictPort: true,
    // If the TAURI_DEV_HOST env var is set (e.g. when running on mobile
    // or from a different host), bind to it; otherwise bind to localhost.
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Tell Vite to ignore the Rust source directory to avoid unnecessary
      // reloads when Rust files change.
      ignored: ["**/src-tauri/**"],
    },
  },
}));
