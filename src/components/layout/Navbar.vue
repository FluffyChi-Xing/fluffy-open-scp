<script setup lang="ts">
import { computed, shallowRef } from 'vue'
import NotificationsPanel from '@/components/notification/NotificationsPanel.vue'
import FFullscreen from '@/components/ui/FFullscreen.vue'
import FPopover from '@/components/ui/FPopover.vue'
import type { HeaderActions } from '@/config/app'

interface Props { title: string; currentPage: string; collapsed: boolean; dark: boolean; locale: 'zh-CN' | 'en-US'; headerActions: HeaderActions }
interface Emits { toggleSidebar: []; toggleTheme: []; toggleLocale: []; openSearch: []; openSettings: [] }
const props = defineProps<Props>()
const emit = defineEmits<Emits>()
const localeLabel = computed(() => props.locale === 'zh-CN' ? 'EN' : '中')
const notificationsOpen = shallowRef(false)
</script>

<template>
  <header class="navbar">
    <div class="navbar-start">
      <button class="icon-button" type="button" :aria-label="$t('shell.collapse')" @click="emit('toggleSidebar')"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16" /></svg></button>
      <RouterLink class="brand" to="/"><span class="brand-mark"><img src="/scp-logo.png" alt="" /></span><span class="brand-name">{{ props.title }}</span></RouterLink>
      <span class="context"><span class="context-dot" aria-hidden="true" />{{ props.currentPage }}</span>
    </div>
    <nav class="navbar-actions" :aria-label="$t('shell.controls')">
      <button v-if="props.headerActions.search" class="search-button" type="button" :aria-label="$t('shell.search')" @click="emit('openSearch')"><svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6"/><path d="m16 16 4 4"/></svg><span>{{ $t('shell.search') }}</span><kbd>⌘ K</kbd></button>
      <button v-if="props.headerActions.language" class="text-button" type="button" :aria-label="$t('shell.language')" @click="emit('toggleLocale')">{{ localeLabel }}</button>
      <button v-if="props.headerActions.theme" class="icon-button" type="button" :aria-label="$t('shell.theme')" @click="emit('toggleTheme')"><svg v-if="props.dark" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"/></svg><svg v-else viewBox="0 0 24 24" aria-hidden="true"><path d="M20.5 14.1A8.5 8.5 0 0 1 9.9 3.5 8.5 8.5 0 1 0 20.5 14.1Z"/></svg></button>
      <FPopover v-if="props.headerActions.notifications" v-model:open="notificationsOpen"><template #trigger><button class="icon-button" type="button" :aria-label="$t('shell.notifications')"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M18 9a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4"/></svg></button></template><NotificationsPanel /></FPopover>
      <FFullscreen v-if="props.headerActions.fullscreen" />
      <button v-if="props.headerActions.settings" class="icon-button" type="button" :aria-label="$t('shell.settings')" @click="emit('openSettings')"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0 .33-1.82A1.65 1.65 0 0 0 3 13h-.09a2 2 0 1 1 0-4H3a1.65 1.65 0 0 0 1.51-1A1.65 1.65 0 0 0 4.6 6.18l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9.25 3H9a1.65 1.65 0 0 0 1-1.51V1.4a2 2 0 1 1 4 0v.09A1.65 1.65 0 0 0 15 3h.25a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 6.18a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09A1.65 1.65 0 0 0 19.4 12"/></svg></button>
    </nav>
  </header>
</template>

<style scoped>
.navbar{align-items:center;background:color-mix(in srgb,var(--surface) 78%,transparent);border-bottom:1px solid color-mix(in srgb,var(--border) 82%,transparent);display:flex;height:56px;justify-content:space-between;padding:0 18px;position:sticky;top:0;z-index:20;-webkit-backdrop-filter:blur(24px) saturate(160%);backdrop-filter:blur(24px) saturate(160%)}.navbar-start,.navbar-actions{align-items:center;display:flex;gap:6px;min-width:0}.icon-button,.text-button,.search-button{align-items:center;background:transparent;border:0;border-radius:var(--radius-sm);color:var(--muted-foreground);cursor:pointer;display:inline-flex;font:inherit;justify-content:center;min-height:34px;transition:background-color 140ms ease,color 140ms ease,scale 140ms ease}.icon-button:hover,.text-button:hover,.search-button:hover{background:var(--surface-hover);color:var(--foreground)}.icon-button:active,.text-button:active,.search-button:active{scale:.96}.icon-button{padding:0;width:34px}.icon-button svg,.search-button svg{fill:none;height:16px;stroke:currentColor;stroke-linecap:round;stroke-linejoin:round;stroke-width:1.7;width:16px}.brand{align-items:center;color:var(--foreground);display:inline-flex;font-size:14px;font-weight:650;gap:8px;min-width:0;text-decoration:none}.brand-mark{align-items:center;background:var(--primary);border-radius:7px;display:inline-flex;height:25px;justify-content:center;overflow:hidden;width:25px}.brand-mark img{height:100%;object-fit:contain;width:100%}.context{align-items:center;border-left:1px solid var(--border);color:var(--muted-foreground);display:inline-flex;font-size:13px;gap:8px;margin-inline-start:7px;padding-inline-start:14px}.context-dot{background:var(--success);border-radius:50%;box-shadow:0 0 0 3px color-mix(in srgb,var(--success) 14%,transparent);height:6px;width:6px}.search-button{border:1px solid var(--border);gap:7px;margin-inline-end:5px;padding:0 8px 0 10px}.search-button span{font-size:13px}.search-button kbd{background:var(--surface-hover);border:1px solid var(--border);border-radius:4px;color:var(--subtle-foreground);font-size:10px;padding:1px 4px}.text-button{font-size:13px;font-weight:650;padding:0 9px}@media(max-width:720px){.navbar{padding:0 12px}.context,.brand-name,.search-button span,.search-button kbd{display:none}.search-button{border:0;margin:0;padding:0;width:34px}.navbar-actions{gap:2px}}@media(max-width:390px){.text-button{padding:0 6px}}
</style>
