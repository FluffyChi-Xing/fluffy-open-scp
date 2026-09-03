import * as echarts from 'echarts/core'

const registeredModules = new Set<unknown>()

export function registerChartModules(modules: readonly unknown[]) {
  const unregistered = modules.filter((module) => !registeredModules.has(module))
  if (unregistered.length === 0) return
  echarts.use(unregistered as Parameters<typeof echarts.use>[0])
  unregistered.forEach((module) => registeredModules.add(module))
}
