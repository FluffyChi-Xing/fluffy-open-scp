import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FEmpty from './FEmpty.vue'

describe('FEmpty', () => {
  it('renders optional content and a decorative default icon', () => {
    const wrapper = mount(FEmpty, { props: { title: '暂无项目', desc: '创建项目后将在这里显示。' } })

    expect(wrapper.classes()).toEqual(expect.arrayContaining(['f-empty', 'f-empty-default', 'f-empty-info']))
    expect(wrapper.find('.f-empty-title').text()).toBe('暂无项目')
    expect(wrapper.find('.f-empty-desc').text()).toBe('创建项目后将在这里显示。')
    expect(wrapper.find('svg').attributes('aria-hidden')).toBe('true')
  })

  it('omits absent text and applies compact status variants', () => {
    const wrapper = mount(FEmpty, { props: { iconName: 'Search', variant: 'compact', status: 'warning' } })

    expect(wrapper.find('.f-empty-title').exists()).toBe(false)
    expect(wrapper.find('.f-empty-desc').exists()).toBe(false)
    expect(wrapper.classes()).toEqual(expect.arrayContaining(['f-empty-compact', 'f-empty-warning']))
    expect(wrapper.find('svg').exists()).toBe(true)
  })

  it('renders recovery actions through the default slot', () => {
    const wrapper = mount(FEmpty, {
      props: { title: '没有结果' },
      slots: { default: '<button type="button">清除筛选</button>' },
    })

    expect(wrapper.find('.f-empty-actions button').text()).toBe('清除筛选')
  })
})
