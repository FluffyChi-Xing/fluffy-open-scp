import { mount } from '@vue/test-utils'
import { CalendarDays } from 'lucide-vue-next'
import { describe, expect, it } from 'vitest'
import FIcon from './FIcon.vue'
import { registerIcons } from '@/lib/icons'

describe('FIcon', () => {
  it('resolves Lucide names, kebab-case names, and Fluffy aliases', () => {
    expect(mount(FIcon, { props: { name: 'FolderOpen' } }).find('svg').exists()).toBe(true)
    expect(mount(FIcon, { props: { name: 'chart-no-axes-combined' } }).find('svg').exists()).toBe(true)
    expect(mount(FIcon, { props: { name: 'dashboard' } }).find('svg').exists()).toBe(true)
  })

  it('applies visual props and labels semantic icons', () => {
    const wrapper = mount(FIcon, {
      props: { name: 'Trash2', color: '#dc2626', size: 20, strokeWidth: 2, ariaLabel: '删除' }
    })

    const icon = wrapper.find('svg')
    expect(icon.attributes('aria-label')).toBe('删除')
    expect(icon.attributes('role')).toBe('img')
    expect(icon.attributes('stroke-width')).toBe('2')
    expect(icon.attributes('style')).toContain('color: #dc2626')
    expect(icon.attributes('style')).toContain('width: 20px')
  })

  it('marks decorative icons as hidden', () => {
    const icon = mount(FIcon, { props: { name: 'Search' } }).find('svg')
    expect(icon.attributes('aria-hidden')).toBe('true')
    expect(icon.attributes('role')).toBeUndefined()
  })

  it('renders app-registered icons via kebab-case names', () => {
    registerIcons({ CalendarDays })
    expect(mount(FIcon, { props: { name: 'calendar-days' } }).find('svg').exists()).toBe(true)
  })
})
