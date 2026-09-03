import { defineComponent, nextTick, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import FSheet from '@/components/ui/FSheet.vue'

const Host = defineComponent({
  components: { FSheet },
  setup() {
    const open = ref(false)
    return { open }
  },
  template: `<FSheet v-model:open="open"><div class="sheet-content">Panel content</div></FSheet>`
})

afterEach(() => {
  document.body.innerHTML = ''
  document.body.style.overflow = ''
})

describe('FSheet', () => {
  it('toggles visibility and locks body scroll', async () => {
    const wrapper = mount(Host)
    const overlay = document.body.querySelector('.f-sheet-overlay') as HTMLElement
    expect(overlay).not.toBeNull()
    expect(overlay.classList.contains('visible')).toBe(false)

    wrapper.vm.open = true
    await nextTick()
    expect(overlay.classList.contains('visible')).toBe(true)
    expect(document.body.style.overflow).toBe('hidden')
    expect(document.body.textContent).toContain('Panel content')

    wrapper.vm.open = false
    await nextTick()
    expect(overlay.classList.contains('visible')).toBe(false)
    expect(document.body.style.overflow).toBe('')
    wrapper.unmount()
  })

  it('closes on overlay self mousedown and on Escape', async () => {
    const wrapper = mount(Host)
    wrapper.vm.open = true
    await nextTick()

    const overlay = document.body.querySelector('.f-sheet-overlay') as HTMLElement
    overlay.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
    await nextTick()
    expect(wrapper.vm.open).toBe(false)

    wrapper.vm.open = true
    await nextTick()
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await nextTick()
    expect(wrapper.vm.open).toBe(false)
    wrapper.unmount()
  })
})
