import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FTypography from './FTypography.vue'

describe('FTypography', () => {
  it('renders semantic headings and paragraphs', () => {
    expect(mount(FTypography, { props: { header: 2 }, slots: { default: '标题' } }).find('h2').text()).toBe('标题')
    expect(mount(FTypography, { props: { paragraphy: true }, slots: { default: '正文' } }).find('p').text()).toBe('正文')
  })

  it('expands and collapses clamped paragraphs', async () => {
    const wrapper = mount(FTypography, {
      props: { paragraphy: true, ellipsis: { rows: 2, expandable: true } },
      slots: { default: '很长的正文' }
    })

    await wrapper.find('button').trigger('click')
    expect(wrapper.find('button').text()).toBe('收起')
    await wrapper.find('button').trigger('click')
    expect(wrapper.find('button').text()).toBe('展开')
  })
})
