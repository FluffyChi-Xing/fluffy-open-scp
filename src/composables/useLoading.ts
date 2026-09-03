import { computed, readonly, shallowRef } from 'vue'

export function useLoading(initial = false) {
  const pendingCount = shallowRef(initial ? 1 : 0)
  const loading = computed(() => pendingCount.value > 0)

  function setLoading(value: boolean) {
    pendingCount.value = value ? Math.max(pendingCount.value, 1) : 0
  }

  async function run<T>(task: () => Promise<T>): Promise<T> {
    pendingCount.value += 1
    try {
      return await task()
    } finally {
      pendingCount.value = Math.max(0, pendingCount.value - 1)
    }
  }

  return { loading: readonly(loading), run, setLoading }
}
