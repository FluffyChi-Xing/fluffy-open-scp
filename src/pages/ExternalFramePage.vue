<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { findExternalRoute } from '@/router/registry'
const route = useRoute()
const entry = computed(() => findExternalRoute(String(route.params.key)))
function openExternal() { if (entry.value) window.open(entry.value.url, '_blank', 'noopener,noreferrer') }
</script>
<template><section class="page"><template v-if="entry"><header><div><p class="eyebrow">{{ $t('external.eyebrow') }}</p><h1>{{ $t(entry.titleKey) }}</h1><p>{{ $t('external.description') }}</p></div><button type="button" @click="openExternal">{{ $t('external.openNewTab') }}</button></header><iframe class="frame" :src="entry.url" :title="$t(entry.titleKey)" referrerpolicy="no-referrer" sandbox="allow-forms allow-popups allow-scripts" /></template><article v-else class="empty"><h1>{{ $t('external.unavailable') }}</h1><RouterLink to="/">{{ $t('notFound.action') }}</RouterLink></article></section></template>
<style scoped>
.page{display:grid;gap:20px;min-height:calc(100vh - 220px)}header{align-items:flex-end;display:flex;gap:20px;justify-content:space-between}.eyebrow{color:var(--primary);font-size:11px;font-weight:750;letter-spacing:.08em;margin:0 0 10px;text-transform:uppercase}h1{font-size:clamp(1.8rem,3vw,2.5rem);letter-spacing:-.045em;margin:0}header p:not(.eyebrow){color:var(--muted-foreground);font-size:14px;margin:10px 0 0}button,.empty a{background:var(--primary);border:0;border-radius:var(--radius-md);color:var(--primary-foreground);cursor:pointer;font-size:13px;font-weight:700;padding:10px 13px;text-decoration:none}.frame{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-lg);height:calc(100vh - 270px);min-height:460px;width:100%}.empty{align-items:center;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-lg);display:flex;flex-direction:column;gap:18px;justify-content:center}@media(max-width:620px){header{align-items:stretch;flex-direction:column}.frame{min-height:400px}}
</style>