<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { NavigationItem } from '@/router/types'

interface Props { open: boolean; items: NavigationItem[] }
interface Emits { close: []; select: [item: NavigationItem] }
const props = defineProps<Props>()
const emit = defineEmits<Emits>()
const { t } = useI18n()
const input = useTemplateRef<HTMLInputElement>('input')
const query = defineModel<string>('query', { default: '' })
const selectedIndex = defineModel<number>('selectedIndex', { default: 0 })
const results = computed(() => props.items.filter((item) => `${t(item.titleKey)} ${item.path ?? ''}`.toLocaleLowerCase().includes(query.value.toLocaleLowerCase())))

function close() { emit('close') }
function choose(item: NavigationItem) { emit('select', item) }
function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') { close(); return }
  if (event.key === 'ArrowDown') { event.preventDefault(); selectedIndex.value = Math.min(selectedIndex.value + 1, results.value.length - 1) }
  if (event.key === 'ArrowUp') { event.preventDefault(); selectedIndex.value = Math.max(selectedIndex.value - 1, 0) }
  if (event.key === 'Enter' && results.value[selectedIndex.value]) { event.preventDefault(); choose(results.value[selectedIndex.value]) }
}
watch(() => props.open, async (open) => { if (!open) return; query.value = ''; selectedIndex.value = 0; await nextTick(); input.value?.focus() })
watch(results, () => { selectedIndex.value = 0 })
window.addEventListener('keydown', onKeydown)
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>
<template><Teleport to="body"><div v-if="props.open" class="command-overlay" @mousedown.self="close"><section class="command" role="dialog" aria-modal="true" :aria-label="$t('shell.search')"><div class="command-input"><svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6"/><path d="m16 16 4 4"/></svg><input ref="input" v-model="query" :placeholder="$t('shell.searchPlaceholder')" /></div><div class="command-results"><button v-for="(item,index) in results" :key="item.key" class="command-item" :class="{ selected:index===selectedIndex }" type="button" @mouseenter="selectedIndex=index" @click="choose(item)"><span>{{ $t(item.titleKey) }}</span><small>{{ item.external ? item.external.url : item.path }}</small></button><p v-if="!results.length" class="command-empty">{{ $t('shell.noResults') }}</p></div><footer>{{ $t('shell.commandHint') }}</footer></section></div></Teleport></template>
<style scoped>
.command-overlay{align-items:flex-start;background:oklch(0.1 0.01 260 / .45);display:flex;inset:0;justify-content:center;padding-top:min(14vh,120px);position:fixed;z-index:80}.command{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow-md);display:grid;grid-template-rows:auto minmax(0,1fr) auto;height:min(480px,calc(100vh - 80px));overflow:hidden;width:min(640px,calc(100vw - 32px))}.command-input{align-items:center;border-bottom:1px solid var(--border);display:flex;gap:10px;padding:13px 15px}.command-input svg{fill:none;height:18px;stroke:var(--muted-foreground);stroke-linecap:round;stroke-width:1.8;width:18px}.command-input input{background:transparent;border:0;color:var(--foreground);font-size:14px;outline:0;width:100%}.command-results{overflow-y:auto;padding:8px}.command-item{background:transparent;border:0;border-radius:var(--radius-sm);color:var(--foreground);cursor:pointer;display:flex;flex-direction:column;gap:3px;padding:10px;text-align:start;width:100%}.command-item:hover,.command-item.selected{background:var(--surface-hover)}.command-item span{font-size:13px;font-weight:650}.command-item small{color:var(--subtle-foreground);font-size:11px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.command-empty{color:var(--muted-foreground);font-size:13px;padding:26px;text-align:center}.command footer{border-top:1px solid var(--border);color:var(--subtle-foreground);font-size:11px;padding:10px 15px}
</style>
