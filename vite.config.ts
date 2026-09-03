import { fileURLToPath, URL } from 'node:url'
import { defineConfig, loadEnv, type ProxyOptions } from 'vite'
import tailwindcss from '@tailwindcss/vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd())
  const proxy: Record<string, ProxyOptions> = {}
  if (env.VITE_FLUFFY_OSS_BASE_URL?.startsWith('/') && env.VITE_FLUFFY_OSS_PROXY_TARGET) {
    proxy[env.VITE_FLUFFY_OSS_BASE_URL] = { target: env.VITE_FLUFFY_OSS_PROXY_TARGET, changeOrigin: true }
  }
  if (env.VITE_FLUFFY_LOG_BASE_URL?.startsWith('/') && env.VITE_FLUFFY_LOG_PROXY_TARGET) {
    proxy[env.VITE_FLUFFY_LOG_BASE_URL] = { target: env.VITE_FLUFFY_LOG_PROXY_TARGET, changeOrigin: true }
  }

  return {
    plugins: [tailwindcss(), vue()],
    resolve: {
      alias: {
        '@': fileURLToPath(new URL('./src', import.meta.url))
      }
    },
    // Tauri: fixed dev port expected by tauri.conf.json, no console clearing
    clearScreen: false,
    envPrefix: ['VITE_', 'TAURI_'],
    server: {
      port: 5173,
      strictPort: true,
      ...(Object.keys(proxy).length > 0 ? { proxy } : {})
    },
    build: {
      target: 'chrome105'
    }
  }
})
