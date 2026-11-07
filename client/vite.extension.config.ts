import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'

const localPath = (path: string) => fileURLToPath(new URL(path, import.meta.url))

export default defineConfig({
  root: localPath('./extension'),
  base: './',
  publicDir: false,
  plugins: [{
    name: 'extension-assets',
    generateBundle() {
      this.emitFile({
        type: 'asset',
        fileName: 'manifest.json',
        source: readFileSync(localPath('./extension/manifest.json')),
      })
      for (const name of ['favicon-16x16.png', 'favicon-32x32.png', 'favicon-48x48.png', 'logo192.png']) {
        this.emitFile({
          type: 'asset',
          fileName: name,
          source: readFileSync(localPath(`./public/${name}`)),
        })
      }
    },
  }],
  build: {
    outDir: localPath('./dist-extension'),
    emptyOutDir: true,
    rollupOptions: {
      input: {
        popup: localPath('./extension/popup.html'),
        background: localPath('./extension/background.ts'),
      },
      output: { entryFileNames: '[name].js' },
    },
  },
})
