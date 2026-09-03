import { describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import { mount } from '@vue/test-utils'
import FFullscreen from '@/components/ui/FFullscreen.vue'

describe('FFullscreen', () => {
  it('toggles fullscreen and reflects state changes', async () => {
    let inFullscreen = false
    Object.defineProperty(document, 'fullscreenElement', { configurable: true, get: () => (inFullscreen ? document.documentElement : null) })
    const requestFullscreen = vi.fn(async () => { inFullscreen = true })
    const exitFullscreen = vi.fn(async () => { inFullscreen = false })
    Object.defineProperty(document.documentElement, 'requestFullscreen', { configurable: true, value: requestFullscreen })
    Object.defineProperty(document, 'exitFullscreen', { configurable: true, value: exitFullscreen })

    const wrapper = mount(FFullscreen, { global: { mocks: { $t: (key: string) => key } } })
    const button = wrapper.find('button')
    expect(button.attributes('aria-label')).toBe('shell.enterFullscreen')

    await button.trigger('click')
    expect(requestFullscreen).toHaveBeenCalledTimes(1)
    document.dispatchEvent(new Event('fullscreenchange'))
    await nextTick()
    expect(button.attributes('aria-label')).toBe('shell.exitFullscreen')

    await button.trigger('click')
    expect(exitFullscreen).toHaveBeenCalledTimes(1)
    document.dispatchEvent(new Event('fullscreenchange'))
    await nextTick()
    expect(button.attributes('aria-label')).toBe('shell.enterFullscreen')
    wrapper.unmount()
  })
})
