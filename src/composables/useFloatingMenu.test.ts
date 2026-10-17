import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, nextTick, ref } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import { useFloatingMenu } from './useFloatingMenu'

describe('floating menu viewport bounds', () => {
  it.each([100, 700])('bounds and scrolls a long menu at y=%i', async (top) => {
    const open = ref(false)
    const anchor = ref<HTMLElement | null>(null)
    const panel = ref<HTMLElement | null>(null)
    const wrapper = mount(defineComponent({
      setup() {
        const { panelStyle } = useFloatingMenu(open, anchor, panel)
        return () => h('div', [
          h('button', { ref: anchor }),
          h('div', { ref: panel, style: panelStyle.value }, [h('span', 'item')]),
        ])
      },
    }), { attachTo: document.body })
    vi.spyOn(anchor.value!, 'getBoundingClientRect').mockReturnValue({
      top, bottom: top + 30, left: 20, right: 220,
    } as DOMRect)
    Object.defineProperty(panel.value!, 'scrollHeight', { value: 2000 })
    open.value = true
    await nextTick()
    await flushPromises()
    expect(panel.value!.style.maxHeight).toBe('320px')
    expect(panel.value!.style.overflowY).toBe('auto')
    expect(top === 700 ? panel.value!.style.bottom : panel.value!.style.top).not.toBe('')
    panel.value!.firstElementChild!.dispatchEvent(new Event('scroll'))
    expect(open.value).toBe(true)
    window.dispatchEvent(new Event('scroll'))
    expect(open.value).toBe(false)
    wrapper.unmount()
    vi.restoreAllMocks()
  })
})
