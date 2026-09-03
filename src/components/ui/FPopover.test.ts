import { defineComponent, nextTick, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import FPopover from '@/components/ui/FPopover.vue'

const Host = defineComponent({
  components: { FPopover },
  setup() {
    const open = ref(false)
    return { open }
  },
  template: `<FPopover v-model:open="open"><template #trigger><button class="trigger">Bell</button></template><div class="popover-content">Notification list</div></FPopover>`
})

afterEach(() => {
  document.body.innerHTML = ''
})

describe('FPopover', () => {
  it('renders teleported content when open', async () => {
    const wrapper = mount(Host)
    expect(document.body.querySelector('.f-popover-panel')).toBeNull()

    await wrapper.find('.trigger').trigger('click')
    await nextTick()
    const panel = document.body.querySelector('.f-popover-panel')
    expect(panel).not.toBeNull()
    expect(panel!.textContent).toContain('Notification list')
    wrapper.unmount()
  })

  it('closes on outside pointerdown and on Escape', async () => {
    const wrapper = mount(Host)
    await wrapper.find('.trigger').trigger('click')
    await nextTick()

    document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }))
    await nextTick()
    expect(document.body.querySelector('.f-popover-panel')).toBeNull()

    await wrapper.find('.trigger').trigger('click')
    await nextTick()
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await nextTick()
    expect(document.body.querySelector('.f-popover-panel')).toBeNull()
    wrapper.unmount()
  })
})
