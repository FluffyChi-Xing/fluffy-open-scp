<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue'
import type { EChartsCoreOption, EChartsInitOpts, EChartsType } from 'echarts/core'
import { useChart } from '@/composables/useChart'

interface Props {
  option: EChartsCoreOption
  modules: unknown[]
  autoresize?: boolean
  theme?: string | object
  initOptions?: EChartsInitOpts
  loading?: boolean
  width?: number | string
  height?: number | string
}

const props = withDefaults(defineProps<Props>(), { autoresize: true, height: 320 })
const emit = defineEmits<{ ready: [instance: EChartsType]; disposed: [] }>()
const element = shallowRef<HTMLElement | null>(null)
const { instance, dispose, resize, setOption } = useChart(element, { modules: props.modules, option: () => props.option, autoresize: props.autoresize, theme: props.theme, initOptions: props.initOptions, onReady: (chart) => emit('ready', chart) })
const style = computed(() => ({ height: typeof props.height === 'number' ? `${props.height}px` : props.height, width: typeof props.width === 'number' ? `${props.width}px` : props.width }))
watch([instance, () => props.loading], ([chart, loading]) => {
  if (!chart) return
  if (loading) chart.showLoading()
  else chart.hideLoading()
}, { immediate: true })
function disposeChart() { dispose(); emit('disposed') }
defineExpose({ instance, dispose: disposeChart, resize, setOption })
</script>

<template><div ref="element" class="f-chart" :style="style" role="img"><slot v-if="props.loading" name="loading" /></div></template>

<style scoped>.f-chart{min-width:0}</style>
