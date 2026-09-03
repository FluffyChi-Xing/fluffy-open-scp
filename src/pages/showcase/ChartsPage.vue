<script setup lang="ts">
import { computed } from 'vue'
import { BarChart, LineChart } from 'echarts/charts'
import { GridComponent, LegendComponent, TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import FChart from '@/components/extensions/FChart.vue'

interface TooltipDatum { axisValueLabel?: string; color?: string; seriesName?: string; value?: number }
const modules = [BarChart, CanvasRenderer, GridComponent, LegendComponent, LineChart, TooltipComponent]

function formatAxisTooltip(params: unknown): string {
  const items = (Array.isArray(params) ? params : [params]) as TooltipDatum[]
  const axisLabel = items[0]?.axisValueLabel ?? ''
  const rows = items.filter((item) => item.seriesName).map((item) => {
    const marker = `<span style="display:inline-block;width:8px;height:8px;border-radius:2px;background:${item.color ?? 'var(--primary)'};margin-right:8px"></span>`
    const value = typeof item.value === 'number' ? item.value.toLocaleString() : String(item.value ?? '')
    return `<div style="display:flex;align-items:center;justify-content:space-between;gap:24px"><span style="display:flex;align-items:center;color:var(--muted-foreground);font-size:13px;line-height:1.8">${marker}${item.seriesName}</span><strong style="color:var(--foreground);font-size:13px;font-weight:600">${value}</strong></div>`
  }).join('')
  return `<div style="background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-md);padding:10px 12px;min-width:150px">${axisLabel ? `<div style="color:var(--subtle-foreground);font-size:11px;font-weight:700;letter-spacing:.08em;margin-bottom:4px;text-transform:uppercase">${axisLabel}</div>` : ''}${rows}</div>`
}

const option = computed(() => ({
  color: ['#4f46e5', '#22a06b'],
  grid: { left: 36, right: 20, top: 44, bottom: 28 },
  legend: { top: 8, textStyle: { color: '#7c8090' } },
  tooltip: { trigger: 'axis', confine: true, backgroundColor: 'transparent', borderColor: 'transparent', borderWidth: 0, padding: 0, formatter: formatAxisTooltip },
  xAxis: { type: 'category', data: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'], axisLine: { lineStyle: { color: '#d8dae0' } } },
  yAxis: { type: 'value', splitLine: { lineStyle: { color: '#eceef2' } } },
  series: [{ name: 'Requests', type: 'line', smooth: true, data: [84, 119, 92, 158, 141, 187, 206] }, { name: 'Deployments', type: 'bar', barMaxWidth: 26, data: [12, 18, 15, 25, 21, 30, 35] }]
}))
</script>

<template><section class="page"><header><p class="eyebrow">{{ $t('showcase.eyebrow') }}</p><h1>{{ $t('showcase.chartsTitle') }}</h1><p>{{ $t('showcase.chartsDescription') }}</p></header><article class="panel"><div class="panel-heading"><div><h2>{{ $t('showcase.traffic') }}</h2><p>{{ $t('showcase.trafficDescription') }}</p></div><span>{{ $t('showcase.lastSevenDays') }}</span></div><FChart :modules="modules" :option="option" height="340px" class="chart" /></article></section></template>

<style scoped>.page{display:grid;gap:24px}.eyebrow{color:var(--primary);font-size:11px;font-weight:750;letter-spacing:.08em;margin:0 0 10px;text-transform:uppercase}.page h1{font-size:clamp(1.8rem,3vw,2.5rem);letter-spacing:-.045em;margin:0}.page header p:not(.eyebrow),.panel-heading p{color:var(--muted-foreground);font-size:14px;margin:10px 0 0}.panel{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow-sm);padding:20px}.panel-heading{align-items:flex-start;display:flex;justify-content:space-between}.panel-heading h2{font-size:14px;margin:0}.panel-heading span{color:var(--subtle-foreground);font-size:12px}.chart{margin-top:16px;width:100%}</style>
