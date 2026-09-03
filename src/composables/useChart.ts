import { onBeforeUnmount, onMounted, shallowRef, toValue, watch } from 'vue'
import type { MaybeRefOrGetter, ShallowRef } from 'vue'
import * as echarts from 'echarts/core'
import type { EChartsCoreOption, EChartsInitOpts, EChartsType } from 'echarts/core'
import { registerChartModules } from '@/lib/chart-modules'

export interface UseChartOptions<Option extends EChartsCoreOption = EChartsCoreOption> {
  modules: unknown[]
  option: MaybeRefOrGetter<Option>
  autoresize?: boolean
  theme?: string | object
  initOptions?: EChartsInitOpts
  onReady?: (instance: EChartsType) => void
}

export function useChart<Option extends EChartsCoreOption>(element: ShallowRef<HTMLElement | null>, options: UseChartOptions<Option>) {
  const instance = shallowRef<EChartsType>()
  let resizeObserver: ResizeObserver | undefined

  function setOption(option = toValue(options.option)) { instance.value?.setOption(option) }
  function resize() { instance.value?.resize() }
  function dispose() {
    resizeObserver?.disconnect()
    resizeObserver = undefined
    instance.value?.dispose()
    instance.value = undefined
  }

  onMounted(() => {
    const target = element.value
    if (!target) return
    registerChartModules(options.modules)
    instance.value = echarts.getInstanceByDom(target) ?? echarts.init(target, options.theme, options.initOptions)
    setOption()
    options.onReady?.(instance.value)
    if (options.autoresize !== false && typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(resize)
      resizeObserver.observe(target)
    }
  })

  watch(() => toValue(options.option), () => setOption(), { deep: true })
  onBeforeUnmount(dispose)
  return { instance, setOption, resize, dispose }
}
