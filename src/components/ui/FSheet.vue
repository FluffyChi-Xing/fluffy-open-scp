<script setup lang="ts">
import { onBeforeUnmount, watch } from 'vue'

interface Props { label?: string }
const props = defineProps<Props>()
const open = defineModel<boolean>('open', { default: false })

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') open.value = false
}
window.addEventListener('keydown', onKeydown)
watch(open, (value) => {
  document.body.style.overflow = value ? 'hidden' : ''
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  document.body.style.overflow = ''
})
</script>

<template><Teleport to="body"><div class="f-sheet-overlay" :class="{ visible: open }" :aria-hidden="open ? undefined : 'true'" @mousedown.self="open = false"><aside class="f-sheet" :class="{ visible: open }" role="dialog" aria-modal="true" :aria-label="props.label"><slot /></aside></div></Teleport></template>

<style scoped>
.f-sheet-overlay{background:oklch(0.1 0.01 260 / .45);inset:0;opacity:0;pointer-events:none;position:fixed;transition:opacity 180ms ease;z-index:80}.f-sheet-overlay.visible{opacity:1;pointer-events:auto}.f-sheet{background:var(--surface);border-left:1px solid var(--border);box-shadow:var(--shadow-md);display:flex;flex-direction:column;height:100%;inset:0 0 0 auto;overflow-y:auto;position:absolute;transform:translateX(100%);transition:transform 200ms ease;width:min(360px,92vw)}.f-sheet.visible{transform:translateX(0)}
</style>
