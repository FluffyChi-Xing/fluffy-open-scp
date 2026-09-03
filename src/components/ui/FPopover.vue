<script setup lang="ts">
import { useTemplateRef } from 'vue'
import { useFloatingMenu } from '@/composables/useFloatingMenu'

interface Props { width?: number }
const props = withDefaults(defineProps<Props>(), { width: 300 })
const open = defineModel<boolean>('open', { default: false })
const anchor = useTemplateRef<HTMLElement>('anchor')
const panel = useTemplateRef<HTMLElement>('panel')
const { panelStyle } = useFloatingMenu(open, anchor, panel, props.width)
</script>

<template><span ref="anchor" class="f-popover-anchor" @click="open = !open"><slot name="trigger" /></span><Teleport to="body"><div v-if="open" ref="panel" class="f-popover-panel" :style="[panelStyle, { width: `${props.width}px` }]"><slot /></div></Teleport></template>

<style scoped>
.f-popover-anchor{display:inline-flex}.f-popover-panel{background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-md);padding:6px;position:fixed;z-index:60}
</style>
