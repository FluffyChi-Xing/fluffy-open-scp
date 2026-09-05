import { onMounted, shallowRef } from 'vue'
import { createDataSource, localDemoConfig } from '@/api/data-source'
import type { OverviewSnapshot } from '@/api/mock-data'

export function useOpenScpOverview() {
  const snapshot = shallowRef<OverviewSnapshot | null>(null)
  const loading = shallowRef(false)
  const error = shallowRef('')
  const demo = shallowRef(localDemoConfig() !== null)
  const source = createDataSource()

  async function load() {
    loading.value = true
    error.value = ''
    try { snapshot.value = await source.overview() } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) } finally { loading.value = false }
  }
  onMounted(load)
  return { snapshot, loading, error, demo, reload: load }
}
