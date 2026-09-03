import { nextTick } from 'vue'
import { mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import FToastHost from '@/components/ui/FToastHost.vue'
import { useToast } from '@/composables/useToast'

describe('FToastHost', () => {
  const toast = useToast()

  beforeEach(() => {
    toast.clear()
    vi.useFakeTimers()
  })

  afterEach(() => {
    toast.clear()
    vi.useRealTimers()
    document.body.innerHTML = ''
  })

  it('renders and dismisses a notification', async () => {
    toast.success('Saved', { title: 'Complete', duration: 100 })
    const wrapper = mount(FToastHost, { global: { mocks: { $t: (key: string) => key } } })
    await vi.dynamicImportSettled()

    expect(document.body.textContent).toContain('Saved')
    expect(document.body.textContent).toContain('Complete')
    ;(document.body.querySelector('.f-toast button') as HTMLButtonElement).click()
    await nextTick()
    expect(toast.toasts.value).toHaveLength(0)
    wrapper.unmount()
  })

  it('automatically dismisses notifications', async () => {
    mount(FToastHost, { attachTo: document.body, global: { mocks: { $t: (key: string) => key } } })
    toast.info('Updated', { duration: 100 })
    await nextTick()

    await vi.advanceTimersByTimeAsync(100)
    expect(toast.toasts.value).toHaveLength(0)
  })
})
