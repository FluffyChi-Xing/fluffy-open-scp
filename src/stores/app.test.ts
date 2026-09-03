import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { appConfig } from '@/config/app'
import { useAppStore } from '@/stores/app'

describe('app store settings', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('seeds settings from appConfig', () => {
    const store = useAppStore()
    expect(store.showNavbar).toBe(appConfig.showNavbar)
    expect(store.showMenu).toBe(appConfig.showMenu)
    expect(store.menuWidth).toBe(appConfig.menuWidth)
    expect(store.colorWeak).toBe(appConfig.colorWeak)
    expect(store.documentTitle).toBe(appConfig.documentTitle)
    expect(store.isTabBarVisible).toBe(appConfig.showTabBar)
  })

  it('updates via setters without persistence', () => {
    const store = useAppStore()
    store.toggleNavbar()
    store.toggleMenu()
    store.setMenuWidth(280)
    store.toggleColorWeak()
    store.toggleDocumentTitle()

    expect(store.showNavbar).toBe(!appConfig.showNavbar)
    expect(store.showMenu).toBe(!appConfig.showMenu)
    expect(store.menuWidth).toBe(280)
    expect(store.colorWeak).toBe(!appConfig.colorWeak)
    expect(store.documentTitle).toBe(!appConfig.documentTitle)
    expect(localStorage.length).toBe(0)
  })
})
