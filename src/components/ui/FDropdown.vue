<script setup lang="ts">
import { useTemplateRef } from 'vue'
import { useFloatingMenu } from '@/composables/useFloatingMenu'

interface Props { width?: number }
const props = withDefaults(defineProps<Props>(), { width: 190 })
const open = defineModel<boolean>('open', { default: false })
const anchor = useTemplateRef<HTMLElement>('anchor')
const panel = useTemplateRef<HTMLElement>('panel')
const { panelStyle } = useFloatingMenu(open, anchor, panel, props.width)
</script>

<template><span ref="anchor" class="f-dropdown-anchor" @click="open = !open"><slot name="trigger" /></span><Teleport to="body"><div v-if="open" ref="panel" class="f-dropdown-panel" :style="[panelStyle, { width: `${props.width}px` }]" role="menu"><slot /></div></Teleport></template>

<style scoped>
.f-dropdown-anchor{display:inline-flex}.f-dropdown-panel{background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-md);padding:5px;position:fixed;z-index:60}.f-dropdown-panel :slotted(button){align-items:center;background:transparent;border:0;border-radius:var(--radius-sm);color:var(--foreground);cursor:pointer;display:flex;font-size:12px;gap:9px;padding:8px 9px;text-align:start;width:100%}.f-dropdown-panel :slotted(button:hover){background:var(--surface-hover)}.f-dropdown-panel :slotted(button.danger){color:var(--danger)}.f-dropdown-panel :slotted(button.danger:hover){background:color-mix(in srgb,var(--danger) 10%,transparent)}.f-dropdown-panel :slotted(button svg){fill:none;height:15px;stroke:currentColor;stroke-linecap:round;stroke-linejoin:round;stroke-width:1.7;width:15px}
</style>
