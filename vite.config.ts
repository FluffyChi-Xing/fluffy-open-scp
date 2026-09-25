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
      host: '127.0.0.1',
      port: 5173,
      strictPort: true,
      // 预热首屏真正会用到的那几个入口，避免第一次请求时才现做转换。
      warmup: {
        clientFiles: [
          './src/main.ts',
          './src/App.vue',
          './src/layouts/DefaultLayout.vue',
          './src/router/index.ts',
          './src/router/registry.ts',
          './src/pages/overview/index.vue',
          './src/pages/studio/index.vue',
          './src/pages/studio/panels/ui.vue'
        ]
      },
      ...(Object.keys(proxy).length > 0 ? { proxy } : {})
    },
    /**
     * 路由已改为动态 import，重型库不再出现在首屏模块图里。这里显式声明，
     * 让 Vite 首次启动就预打包好，而不是等用户点到那一页才触发
     * 「发现新依赖 → 重新优化 → 整页刷新」的二次卡顿。
     */
    optimizeDeps: {
      include: [
        'three',
        'three/examples/jsm/controls/TransformControls.js',
        '@vue-flow/core',
        '@vue-flow/background',
        'echarts/core',
        'echarts/charts',
        'echarts/components',
        'echarts/renderers',
        'shiki',
        'markdown-it'
      ]
    },
    build: {
      target: 'chrome105'
    }
  }
})
