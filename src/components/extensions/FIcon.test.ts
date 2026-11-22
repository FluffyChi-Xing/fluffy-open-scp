import { mount } from '@vue/test-utils'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { CalendarDays } from 'lucide-vue-next'
import { describe, expect, it } from 'vitest'
import FIcon from './FIcon.vue'
import { registerIcons, resolveIcon } from '@/lib/icons'

describe('FIcon', () => {
  it('resolves Lucide names, kebab-case names, and Fluffy aliases', () => {
    expect(mount(FIcon, { props: { name: 'FolderOpen' } }).find('svg').exists()).toBe(true)
    expect(mount(FIcon, { props: { name: 'chart-no-axes-combined' } }).find('svg').exists()).toBe(true)
    expect(mount(FIcon, { props: { name: 'dashboard' } }).find('svg').exists()).toBe(true)
  })

  it('resolves workbench focus and fullscreen controls from the built-in registry', () => {
    for (const name of ['PanelTop', 'PanelBottom', 'Maximize2', 'Minimize2']) {
      expect(resolveIcon(name)).toBeDefined()
    }
  })

  it('renders the building panel tab icon instead of the help fallback', () => {
    const wrapper = mount(FIcon, { props: { name: 'PanelsTopLeft' } })
    expect(wrapper.find('svg').classes().some(name => name.startsWith('lucide-panels-top-left'))).toBe(true)
    expect(wrapper.find('svg').classes().some(name => name.startsWith('lucide-circle-help'))).toBe(false)
  })

  it('registers every icon used by the package configuration tabs', () => {
    const source = readFileSync(resolve('src/pages/studio/components/asset/PackageProjectForm.vue'), 'utf8')
    const tabs = source.match(/const TABS:[\s\S]*?=\s*\[([\s\S]*?)\];/)?.[1]
    expect(tabs).toBeDefined()
    const names = [...tabs!.matchAll(/icon:\s*"([^"]+)"/g)].map(match => match[1])
    expect(names.length).toBeGreaterThan(5)
    for (const name of names) expect(resolveIcon(name), name).toBeDefined()
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
