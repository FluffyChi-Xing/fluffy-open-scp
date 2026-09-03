import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import FCode from '@/components/ui/FCode.vue'

const originalClipboard = navigator.clipboard

afterEach(() => {
  Object.defineProperty(navigator, 'clipboard', { value: originalClipboard, configurable: true })
})

describe('FCode', () => {
  it('renders the language label and highlighted code', async () => {
    const wrapper = mount(FCode, { props: { code: 'const a = 1', lang: 'ts' } })
    await flushPromises()
    expect(wrapper.find('.f-code-lang').text()).toBe('ts')
    expect(wrapper.find('.f-code-shiki code').text()).toBe('const a = 1')
  })

  it('collapses the code body when a traffic-light dot is clicked', async () => {
    const wrapper = mount(FCode, { props: { code: 'const a = 1' } })
    const body = wrapper.find('.f-code-body')
    expect(body.isVisible()).toBe(true)
    await wrapper.find('.f-code-dot-green').trigger('click')
    expect(wrapper.find('.f-code').classes()).toContain('collapsed')
    expect(body.attributes('style')).toContain('display: none')
  })

  it('copies the code to the clipboard', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    const wrapper = mount(FCode, { props: { code: 'const a = 1', copiedLabel: 'Copied' } })
    await wrapper.find('.f-code-copy').trigger('click')
    await flushPromises()
    expect(writeText).toHaveBeenCalledWith('const a = 1')
    expect(wrapper.find('.f-code-copy').text()).toContain('Copied')
  })
})
