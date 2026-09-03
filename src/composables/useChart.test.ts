import { defineComponent, h, nextTick, shallowRef } from 'vue'
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const echarts = vi.hoisted(() => {
  const setOption = vi.fn()
  const resize = vi.fn()
  const dispose = vi.fn()
  return {
    dispose,
    getInstanceByDom: vi.fn(() => undefined),
    init: vi.fn(() => ({ setOption, resize, dispose })),
    resize,
    setOption,
    use: vi.fn()
  }
})
const observe = vi.fn()
const disconnect = vi.fn()

vi.mock('echarts/core', () => ({
  getInstanceByDom: echarts.getInstanceByDom,
  init: echarts.init,
  use: echarts.use
}))

import { useChart } from '@/composables/useChart'

class ResizeObserverMock {
  observe = observe
  disconnect = disconnect
}

describe('useChart', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    globalThis.ResizeObserver = ResizeObserverMock as unknown as typeof ResizeObserver
  })

  it('initializes, updates, resizes, and disposes the chart', async () => {
    const option = shallowRef({})
    const modules = ['module']
    const ChartFixture = defineComponent({
      setup() {
        const element = shallowRef<HTMLElement | null>(null)
        useChart(element, { modules, option })
        return () => h('div', { ref: element })
      }
    })

    const wrapper = mount(ChartFixture)
    expect(echarts.use).toHaveBeenCalledWith(modules)
    expect(echarts.init).toHaveBeenCalledOnce()
    expect(echarts.setOption).toHaveBeenCalledWith(option.value)
    expect(observe).toHaveBeenCalledOnce()

    option.value = { backgroundColor: '#fff' }
    await nextTick()
    expect(echarts.setOption).toHaveBeenLastCalledWith(option.value)

    wrapper.unmount()
    expect(disconnect).toHaveBeenCalledOnce()
    expect(echarts.dispose).toHaveBeenCalledOnce()
  })

  it('does not observe resize when autoresize is disabled', () => {
    const ChartFixture = defineComponent({
      setup() {
        const element = shallowRef<HTMLElement | null>(null)
        useChart(element, { modules: [], option: {}, autoresize: false })
        return () => h('div', { ref: element })
      }
    })

    mount(ChartFixture)
    expect(observe).not.toHaveBeenCalled()
  })
})
