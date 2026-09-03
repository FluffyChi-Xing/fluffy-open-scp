import { computed, shallowRef } from 'vue'
import { defineStore } from 'pinia'
import { appConfig } from '@/config/app'

export const useAppStore = defineStore('app', () => {
  const isSidebarCollapsed = shallowRef(false)
  const isDark = shallowRef(appConfig.defaultDarkMode)
  const locale = shallowRef<'zh-CN' | 'en-US'>(appConfig.defaultLocale)
  const isTabBarVisible = shallowRef(appConfig.showTabBar)
  const showNavbar = shallowRef(appConfig.showNavbar)
  const showMenu = shallowRef(appConfig.showMenu)
  const menuWidth = shallowRef(appConfig.menuWidth)
  const colorWeak = shallowRef(appConfig.colorWeak)
  const documentTitle = shallowRef(appConfig.documentTitle)

  const documentTheme = computed(() => isDark.value ? 'dark' : 'light')

  function toggleSidebar() {
    isSidebarCollapsed.value = !isSidebarCollapsed.value
  }

  function toggleTheme() {
    isDark.value = !isDark.value
  }

  function toggleLocale() {
    locale.value = locale.value === 'zh-CN' ? 'en-US' : 'zh-CN'
  }

  function toggleTabBar() {
    isTabBarVisible.value = !isTabBarVisible.value
  }

  function toggleNavbar() {
    showNavbar.value = !showNavbar.value
  }

  function toggleMenu() {
    showMenu.value = !showMenu.value
  }

  function setMenuWidth(width: number) {
    menuWidth.value = width
  }

  function toggleColorWeak() {
    colorWeak.value = !colorWeak.value
  }

  function toggleDocumentTitle() {
    documentTitle.value = !documentTitle.value
  }

  return {
    isSidebarCollapsed, isDark, locale, isTabBarVisible, showNavbar, showMenu, menuWidth, colorWeak, documentTitle, documentTheme,
    toggleSidebar, toggleTheme, toggleLocale, toggleTabBar, toggleNavbar, toggleMenu, setMenuWidth, toggleColorWeak, toggleDocumentTitle
  }
})
