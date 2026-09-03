import { nextTick } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import FMarkdown from '@/components/markdown/FMarkdown.vue'

vi.mock('vue-i18n', () => ({
  useI18n: () => ({ t: (key: string) => key })
}))

describe('FMarkdown', () => {
  it('renders markdown and hydrates code blocks into FCode', async () => {
    const source = ['# Title', '', 'A paragraph.', '', '```ts', 'const a = 1', '```'].join('\n')
    const wrapper = mount(FMarkdown, { props: { source } })
    await nextTick()
    await flushPromises()

    expect(wrapper.find('h1').text()).toBe('Title')
    expect(wrapper.find('.f-markdown > p').text()).toBe('A paragraph.')
    expect(wrapper.find('.f-code').exists()).toBe(true)
    expect(wrapper.find('.f-code-lang').text()).toBe('ts')
    expect(wrapper.find('.f-code-shiki code').text()).toBe('const a = 1')
  })

  it('re-renders code blocks when the source changes', async () => {
    const wrapper = mount(FMarkdown, { props: { source: '# One' } })
    await nextTick()
    await wrapper.setProps({ source: ['# Two', '', '```ts', 'const b = 2', '```'].join('\n') })
    await nextTick()
    await flushPromises()

    expect(wrapper.find('h1').text()).toBe('Two')
    expect(wrapper.find('.f-code').exists()).toBe(true)
    expect(wrapper.find('.f-code-shiki code').text()).toBe('const b = 2')
  })
})
