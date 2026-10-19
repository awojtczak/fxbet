import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'

// `--mode pages` builds the static GitHub Pages site: served under /fxbet/, with the Rust API
// compiled to WebAssembly (../wasm/pkg) instead of a server.
export default defineConfig(({ mode }) => ({
  base: mode === 'pages' ? '/fxbet/' : '/',
  plugins: [react()],
  resolve: {
    alias: {
      'fxbet-wasm': fileURLToPath(
        new URL(mode === 'pages' ? '../wasm/pkg/fxbet_wasm.js' : './src/local/wasm-stub.ts', import.meta.url),
      ),
    },
  },
  build: { chunkSizeWarningLimit: 1000 },
  server: {
    proxy: { '/api': process.env.FXBET_API ?? 'http://127.0.0.1:8088' },
  },
}))
