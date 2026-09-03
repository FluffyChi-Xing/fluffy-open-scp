import { defineComponent, nextTick, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import FDropdown from '@/components/ui/FDropdown.vue'

const Host = defineComponent({
  components: { FDropdown },
  setup() {
    const open = ref(false)
    return { open }
  },
  template: `<FDropdown v-model:open="open"><template #trigger><button class="trigger">Open</button></template><button class="menu-item">Profile</button><button class="menu-item danger">Log out</button></FDropdown>`
})

afterEach(() => {
  document.body.innerHTML = ''
})

describe('FDropdown', () => {
  it('opens and toggles via the trigger', async () => {
    const wrapper = mount(Host)
    expect(document.body.querySelector('.f-dropdown-panel')).toBeNull()

    await wrapper.find('.trigger').trigger('click')
    await nextTick()
    const panel = document.body.querySelector('.f-dropdown-panel')
    expect(panel).not.toBeNull()
    expect(panel!.textContent).toContain('Profile')

    await wrapper.find('.trigger').trigger('click')
    await nextTick()
    expect(document.body.querySelector('.f-dropdown-panel')).toBeNull()
    wrapper.unmount()
  })

  it('closes on outside pointerdown', async () => {
    const wrapper = mount(Host)
    await wrapper.find('.trigger').trigger('click')
    await nextTick()
    expect(document.body.querySelector('.f-dropdown-panel')).not.toBeNull()

    document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }))
    await nextTick()
    expect(document.body.querySelector('.f-dropdown-panel')).toBeNull()
    wrapper.unmount()
  })

  it('closes on Escape', async () => {
    const wrapper = mount(Host)
    await wrapper.find('.trigger').trigger('click')
    await nextTick()

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await nextTick()
    expect(document.body.querySelector('.f-dropdown-panel')).toBeNull()
    wrapper.unmount()
  })
})
