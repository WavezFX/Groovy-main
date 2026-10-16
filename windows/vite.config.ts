import { defineConfig } from "vite";
import { resolve } from "node:path";

// Groovy's sounds are made in code (src/core/sounds-synth.ts): nothing to serve or copy.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    // Cargo writes and locks files under target/ while Vite starts; watching them
    // crashes Vite on Windows with EBUSY. Tauri watches src-tauri/ itself.
    watch: { ignored: ["**/target/**", "**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "chrome110",
    minify: "esbuild",
    sourcemap: false,
    emptyOutDir: true,
    rollupOptions: {
      input: {
        island: resolve(__dirname, "index.html"),
        settings: resolve(__dirname, "settings.html"),
        mochi: resolve(__dirname, "mochi.html"),
        studio: resolve(__dirname, "studio.html"),
      },
    },
  },
});
