import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Tauri standard config: serve frontendDist as root origin.
export default defineConfig({
  plugins: [react()],
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
