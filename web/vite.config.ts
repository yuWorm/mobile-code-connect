import path from 'node:path'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

const controlProxyTarget = process.env.VITE_CONTROL_API_PROXY_TARGET ?? 'http://127.0.0.1:4242'
const controlApiPrefixes = [
  '/auth',
  '/oauth',
  '/server-auth',
  '/server-credentials',
  '/dashboard',
  '/audit-logs',
  '/usage',
  '/controllers',
  '/users',
  '/devices',
  '/plans',
  '/relay-credentials',
  '/relay-bootstraps',
  '/relays',
  '/mobile',
  '/sessions',
]

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    proxy: Object.fromEntries(
      controlApiPrefixes.map((prefix) => [
        prefix,
        {
          target: controlProxyTarget,
          changeOrigin: true,
        },
      ]),
    ),
  },
})
