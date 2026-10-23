import { defineConfig } from 'vite'
import solid from '@solidjs/vite-plugin'

// Tauri standard config: serve frontendDist as root origin.
export default defineConfig({
  plugins: [solid()],
  clearScreen: false,
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
  },
  server: {
    port: 5173,
    strictPort: true,
  },
})
