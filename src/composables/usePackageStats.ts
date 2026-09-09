import { shallowRef, watch } from 'vue'
import { createDataSource } from '@/api/data-source'
import type { PackageStatistics } from '@/api/tauri'

/** 首页统计卡片：对历史打开（或指定）的 package 做扩展名数量/容量统计。 */
export function usePackageStats(paths: () => string[]) {
  const stats = shallowRef<PackageStatistics | null>(null)
  const loading = shallowRef(false)
  const error = shallowRef('')
  const source = createDataSource()

  async function load() {
    const list = paths().filter((path) => path.toLowerCase().endsWith('.package'))
    if (list.length === 0) {
      stats.value = null
      return
    }
    loading.value = true
    error.value = ''
    try {
      stats.value = await source.packageStats(list)
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
    } finally {
      loading.value = false
    }
  }
  watch(paths, load, { immediate: true })
  return { stats, loading, error, reload: load }
}
