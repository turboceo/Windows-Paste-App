import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// base must be relative: the built page is served from a WebView2 virtual host
// root, and Vite's default absolute "/assets/..." paths would 404 there.
export default defineConfig({
  plugins: [vue()],
  base: './',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // Keep the output inspectable; the host copies dist next to the exe.
    sourcemap: false,
  },
  server: {
    port: 5173,
    strictPort: true,
  },
})
