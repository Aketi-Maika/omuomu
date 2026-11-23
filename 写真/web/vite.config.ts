import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// base を相対にしておくと、GitHub Pages（/omuomu/）でもローカル管理画面（/）でも同じビルドで動く
export default defineConfig({
  base: './',
  plugins: [vue()],
  build: { target: 'es2020', assetsInlineLimit: 0 },
  server: {
    // npm run dev 中もローカル管理ツールの API を使えるように
    proxy: { '/api': 'http://127.0.0.1:8787' },
  },
})
