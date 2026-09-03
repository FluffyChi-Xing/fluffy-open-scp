import { computed, shallowRef } from 'vue'
import { defineStore } from 'pinia'

export interface AppTab {
  fullPath: string
  titleKey: string
  closable: boolean
}

export const useTabStore = defineStore('tabs', () => {
  const tabs = shallowRef<AppTab[]>([])
  const activePath = shallowRef('')
  const activeTab = computed(() => tabs.value.find((tab) => tab.fullPath === activePath.value))

  function openTab(tab: AppTab) {
    if (!tabs.value.some((item) => item.fullPath === tab.fullPath)) tabs.value = [...tabs.value, tab]
    activePath.value = tab.fullPath
  }

  function removeTabs(predicate: (tab: AppTab, index: number) => boolean) {
    tabs.value = tabs.value.filter((tab, index) => !tab.closable || !predicate(tab, index))
  }

  function closeTab(fullPath: string): string | undefined {
    const index = tabs.value.findIndex((tab) => tab.fullPath === fullPath)
    if (index < 0 || !tabs.value[index].closable) return undefined
    const nextTabs = tabs.value.filter((tab) => tab.fullPath !== fullPath)
    tabs.value = nextTabs
    if (activePath.value !== fullPath) return undefined
    const nextTab = nextTabs[index - 1] ?? nextTabs[index]
    activePath.value = nextTab?.fullPath ?? ''
    return nextTab?.fullPath
  }

  function closeLeftTabs(fullPath: string) {
    const index = tabs.value.findIndex((tab) => tab.fullPath === fullPath)
    if (index < 1) return
    removeTabs((_tab, tabIndex) => tabIndex < index)
    activePath.value = fullPath
  }

  function closeRightTabs(fullPath: string) {
    const index = tabs.value.findIndex((tab) => tab.fullPath === fullPath)
    if (index < 0) return
    removeTabs((_tab, tabIndex) => tabIndex > index)
    activePath.value = fullPath
  }

  function closeOtherTabs(fullPath: string) {
    removeTabs((tab) => tab.fullPath !== fullPath)
    activePath.value = fullPath
  }

  function closeAllTabs() {
    tabs.value = tabs.value.filter((tab) => !tab.closable)
    activePath.value = tabs.value[0]?.fullPath ?? ''
    return activePath.value
  }

  return { tabs, activePath, activeTab, openTab, closeTab, closeLeftTabs, closeRightTabs, closeOtherTabs, closeAllTabs }
})
