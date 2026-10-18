import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  build: { chunkSizeWarningLimit: 1000 },
  server: {
    proxy: { '/api': process.env.FXBET_API ?? 'http://127.0.0.1:8088' },
  },
})
