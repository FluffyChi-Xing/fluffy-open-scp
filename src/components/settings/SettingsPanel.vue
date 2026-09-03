<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import FCheckbox from '@/components/ui/FCheckbox.vue'
import FSelect, { type SelectOption } from '@/components/ui/FSelect.vue'
import { appConfig } from '@/config/app'
import { useAppStore } from '@/stores/app'

const { t } = useI18n()
const app = useAppStore()
const menuWidthOptions: SelectOption[] = [
  { label: t('panel.width220'), value: '220' },
  { label: t('panel.width244'), value: '244' },
  { label: t('panel.width280'), value: '280' }
]
const menuWidth = computed({
  get: () => String(app.menuWidth),
  set: (value: string) => app.setMenuWidth(Number(value))
})
</script>

<template><div class="settings-panel"><header class="settings-header"><div><h2>{{ $t('panel.title') }}</h2><p>{{ appConfig.name }}</p></div></header><section class="settings-section"><h3>{{ $t('panel.layout') }}</h3><label class="settings-row"><span class="settings-row-text"><strong>{{ $t('panel.tabBar') }}</strong></span><FCheckbox v-model="app.isTabBarVisible" /></label><label class="settings-row"><span class="settings-row-text"><strong>{{ $t('panel.navbar') }}</strong></span><FCheckbox v-model="app.showNavbar" /></label><label class="settings-row"><span class="settings-row-text"><strong>{{ $t('panel.menu') }}</strong></span><FCheckbox v-model="app.showMenu" /></label><label class="settings-row"><span class="settings-row-text"><strong>{{ $t('panel.menuWidth') }}</strong></span><FSelect class="settings-select" :options="menuWidthOptions" v-model="menuWidth" /></label></section><section class="settings-section"><h3>{{ $t('panel.appearance') }}</h3><label class="settings-row"><span class="settings-row-text"><strong>{{ $t('panel.colorWeak') }}</strong><small>{{ $t('panel.colorWeakHint') }}</small></span><FCheckbox v-model="app.colorWeak" /></label><label class="settings-row"><span class="settings-row-text"><strong>{{ $t('panel.documentTitle') }}</strong><small>{{ $t('panel.documentTitleHint') }}</small></span><FCheckbox v-model="app.documentTitle" /></label></section></div></template>

<style scoped>
.settings-panel{display:flex;flex-direction:column;gap:22px;padding:20px}.settings-header h2{font-size:15px;margin:0}.settings-header p{color:var(--subtle-foreground);font-size:12px;margin:3px 0 0}.settings-section{display:flex;flex-direction:column;gap:2px}.settings-section h3{color:var(--subtle-foreground);font-size:11px;font-weight:700;letter-spacing:.08em;margin:0 0 6px;text-transform:uppercase}.settings-row{align-items:center;border-radius:var(--radius-sm);display:flex;gap:14px;justify-content:space-between;min-height:46px;padding:0 8px}.settings-row:hover{background:var(--surface-hover)}.settings-row-text{display:flex;flex-direction:column;gap:2px}.settings-row-text strong{color:var(--foreground);font-size:13px;font-weight:600}.settings-row-text small{color:var(--subtle-foreground);font-size:11px}.settings-select{width:120px}
</style>
