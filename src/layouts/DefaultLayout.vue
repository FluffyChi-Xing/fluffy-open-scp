<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import Navbar from '@/components/layout/Navbar.vue'
import SidebarNav from '@/components/layout/SidebarNav.vue'
import TabBar from '@/components/layout/TabBar.vue'
import CommandPalette from '@/components/navigation/CommandPalette.vue'
import ChatAssistantDock from '@/integrations/chat-assistant/ChatAssistantDock.vue'
import SettingsPanel from '@/components/settings/SettingsPanel.vue'
import FSheet from '@/components/ui/FSheet.vue'
import { appConfig } from '@/config/app'
import { leafNavigationItems, navigationGroups } from '@/router/registry'
import type { NavigationItem } from '@/router/types'
import { useAppStore } from '@/stores/app'
import { useTabStore, type AppTab } from '@/stores/tabs'

const app = useAppStore()
const tabs = useTabStore()
const route = useRoute()
const router = useRouter()
const { locale, t } = useI18n()
const isMobile = shallowRef(false)
const isMobileNavOpen = shallowRef(false)
const isSearchOpen = shallowRef(false)
const isSettingsOpen = shallowRef(false)
const viewRevision = shallowRef(0)
const mobileMediaQuery = window.matchMedia('(max-width: 720px)')
const currentPage = computed(() => t(String(route.meta.titleKey ?? 'navigation.home')))
const sidebarClass = computed(() => ({ sidebar: true, 'sidebar-collapsed': app.isSidebarCollapsed, 'mobile-open': isMobileNavOpen.value }))
const appShellStyle = computed(() => ({ '--menu-width': `${app.menuWidth}px` }))

function routeTab(): AppTab { return { fullPath: route.fullPath, titleKey: String(route.meta.titleKey ?? 'navigation.home'), closable: !route.meta.noAffix } }
function syncTabs() { tabs.openTab(routeTab()) }
function activateTab(path: string) { router.push(path) }
function closeCurrentTab(path: string) { const nextPath = tabs.closeTab(path); if (nextPath) router.push(nextPath) }
function closeLeftTabs(path: string) { tabs.closeLeftTabs(path); router.push(path) }
function closeRightTabs(path: string) { tabs.closeRightTabs(path); router.push(path) }
function closeOtherTabs(path: string) { tabs.closeOtherTabs(path); router.push(path) }
function closeAllTabs() { const nextPath = tabs.closeAllTabs(); if (nextPath) router.push(nextPath) }
function reloadTab(path: string) { if (route.fullPath !== path) { router.push(path).then(() => { viewRevision.value += 1 }); return }; viewRevision.value += 1 }
function syncMobileState(event: MediaQueryListEvent | MediaQueryList) { isMobile.value = event.matches; if (!event.matches) isMobileNavOpen.value = false }
function toggleNavigation() { if (isMobile.value) { isMobileNavOpen.value = !isMobileNavOpen.value; return }; app.toggleSidebar() }
function closeMobileNavigation() { isMobileNavOpen.value = false }
function openExternal(item: NavigationItem) {
  const external = item.external
  if (!external) return
  if (external.openMode === 'new-tab') { window.open(external.url, '_blank', 'noopener,noreferrer'); closeMobileNavigation(); return }
  router.push({ name: 'external-frame', params: { key: external.key } })
  closeMobileNavigation()
}
function selectNavigation(item: NavigationItem) {
  if (item.external) openExternal(item)
  else if (item.routeName) { router.push({ name: item.routeName }); closeMobileNavigation() }
  isSearchOpen.value = false
}
function onKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); isSearchOpen.value = true; return }
  if (event.key === 'Escape') { if (isSearchOpen.value) { isSearchOpen.value = false; return }; closeMobileNavigation() }
}

onMounted(() => { syncMobileState(mobileMediaQuery); mobileMediaQuery.addEventListener('change', syncMobileState); window.addEventListener('keydown', onKeydown) })
onBeforeUnmount(() => { mobileMediaQuery.removeEventListener('change', syncMobileState); window.removeEventListener('keydown', onKeydown); document.body.style.overflow = '' })
watch([isMobileNavOpen, isSearchOpen], ([navOpen, searchOpen]) => { document.body.style.overflow = navOpen || searchOpen ? 'hidden' : '' })
watch(() => route.fullPath, syncTabs, { immediate: true })
watch(() => app.documentTheme, (theme) => { document.documentElement.dataset.theme = theme }, { immediate: true })
watch(() => app.locale, (nextLocale) => { locale.value = nextLocale; document.documentElement.lang = nextLocale }, { immediate: true })
watch(() => app.colorWeak, (value) => { document.documentElement.classList.toggle('color-weak', value) }, { immediate: true })
</script>
<template><div class="app-shell" :class="{ 'no-navbar': !app.showNavbar }" :style="appShellStyle"><Navbar v-if="app.showNavbar" :title="appConfig.name" :current-page="currentPage" :collapsed="app.isSidebarCollapsed" :dark="app.isDark" :locale="app.locale" :header-actions="appConfig.headerActions" @toggle-sidebar="toggleNavigation" @toggle-theme="app.toggleTheme" @toggle-locale="app.toggleLocale" @open-search="isSearchOpen=true" @open-settings="isSettingsOpen=true" @logout="router.push('/login')"/><CommandPalette :open="isSearchOpen" :items="leafNavigationItems" @close="isSearchOpen=false" @select="selectNavigation"/><button v-if="isMobile && isMobileNavOpen" class="drawer-overlay" type="button" :aria-label="$t('shell.closeNavigation')" @click="closeMobileNavigation"/><div class="shell-body"><aside v-if="app.showMenu" :class="sidebarClass" :aria-label="$t('shell.navigation')"><SidebarNav :groups="navigationGroups" @navigate="closeMobileNavigation" @open-external="openExternal"/><div class="sidebar-footer"><span class="status-dot" aria-hidden="true" />{{ $t('shell.allSystemsOperational') }}</div></aside><main class="page-content"><div class="page-tools"><TabBar v-if="app.isTabBarVisible" :tabs="tabs.tabs" :active-path="route.fullPath" @activate="activateTab" @reload="reloadTab" @close-current="closeCurrentTab" @close-left="closeLeftTabs" @close-right="closeRightTabs" @close-others="closeOtherTabs" @close-all="closeAllTabs"/></div><div class="content-frame"><RouterView v-slot="{ Component }"><component :is="Component" :key="`${route.fullPath}:${viewRevision}`"/></RouterView></div><footer class="footer"><span>{{ appConfig.name }}</span><span>{{ $t('shell.footerStatus') }}</span></footer></main></div><FSheet v-model:open="isSettingsOpen" :label="$t('panel.title')"><SettingsPanel /></FSheet><ChatAssistantDock /></div></template>
<style scoped>
.app-shell{min-height:100vh}.shell-body{display:flex;min-height:calc(100vh - 56px)}.no-navbar .shell-body{min-height:100vh}.sidebar{background:var(--sidebar);border-right:1px solid var(--border);display:flex;flex-direction:column;padding:14px 10px 12px;transition:width 160ms ease,padding 160ms ease;width:var(--menu-width,244px)}.sidebar-collapsed{overflow:hidden;padding-inline:0;width:0}.sidebar-footer{align-items:center;border-top:1px solid var(--border);color:var(--subtle-foreground);display:flex;font-size:11px;gap:7px;margin-top:12px;padding:13px 6px 0}.status-dot{background:var(--success);border-radius:50%;box-shadow:0 0 0 3px color-mix(in srgb,var(--success) 14%,transparent);height:6px;width:6px}.page-content{display:flex;flex:1;flex-direction:column;min-width:0}.page-tools{flex:none}.content-frame{margin-inline:auto;max-width:1180px;padding:36px clamp(20px,4vw,56px) 0;width:100%}.footer{color:var(--subtle-foreground);display:flex;font-size:11px;justify-content:space-between;margin-inline:auto;margin-top:auto;max-width:1180px;padding:40px clamp(20px,4vw,56px) 20px;width:100%}.drawer-overlay{background:oklch(0.1 0.01 260 / .45);border:0;inset:56px 0 0;padding:0;position:fixed;z-index:29}.no-navbar .drawer-overlay{inset:0}@media(max-width:720px){.sidebar{box-shadow:var(--shadow-md);height:calc(100vh - 56px);inset:56px auto 0 0;position:fixed;transform:translateX(-102%);transition:transform 160ms ease;width:min(288px,84vw);z-index:30}.sidebar.sidebar-collapsed{padding:14px 10px 12px;width:min(288px,84vw)}.sidebar.mobile-open{transform:translateX(0)}.no-navbar .sidebar{height:100vh;inset:0 auto 0 0}.content-frame{padding:24px 16px 0}.footer{padding:32px 16px 18px}}@media(prefers-reduced-motion:reduce){.sidebar{transition:none}}
</style>
