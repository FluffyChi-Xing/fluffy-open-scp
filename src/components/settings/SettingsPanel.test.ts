import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import SettingsPanel from '@/components/settings/SettingsPanel.vue'
import { useAppStore } from '@/stores/app'

vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))

describe('SettingsPanel', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('wires toggles and select to the app store', async () => {
    const store = useAppStore()
    const wrapper = mount(SettingsPanel, { global: { mocks: { $t: (key: string) => key } } })

    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    expect(checkboxes).toHaveLength(5)

    await checkboxes[1].setValue(false)
    expect(store.showNavbar).toBe(false)

    await checkboxes[3].setValue(true)
    expect(store.colorWeak).toBe(true)

    await wrapper.find('select').setValue('280')
    expect(store.menuWidth).toBe(280)
    wrapper.unmount()
  })
})
