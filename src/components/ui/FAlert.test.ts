import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FAlert from '@/components/ui/FAlert.vue'

describe('FAlert', () => {
  it('renders default slot with info tone by default', () => {
    const wrapper = mount(FAlert, { slots: { default: 'Diagnostics text' } })
    const alert = wrapper.find('.f-alert')
    expect(alert.exists()).toBe(true)
    expect(alert.attributes('data-type')).toBe('info')
    expect(alert.text()).toContain('Diagnostics text')
    expect(alert.find('.f-alert-close').exists()).toBe(false)
  })

  it('hides after close click and emits close', async () => {
    const wrapper = mount(FAlert, {
      props: { closable: true, closeLabel: 'Dismiss' },
      slots: { default: 'Banner' },
    })
    const button = wrapper.find('.f-alert-close')
    expect(button.exists()).toBe(true)
    expect(button.attributes('aria-label')).toBe('Dismiss')
    await button.trigger('click')
    expect(wrapper.emitted('close')).toHaveLength(1)
    expect(wrapper.find('.f-alert').exists()).toBe(false)
  })

  it('maps tone token and renders action slot', () => {
    const wrapper = mount(FAlert, {
      props: { type: 'warning' },
      slots: { default: 'Warning', action: '<button class="act">More</button>' },
    })
    const alert = wrapper.find('.f-alert')
    expect(alert.attributes('data-type')).toBe('warning')
    expect(alert.attributes('style')).toContain('--f-alert-tone: var(--warning)')
    expect(alert.find('.act').exists()).toBe(true)
  })
})
