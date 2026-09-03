<script setup lang="ts">
import { useFullscreen } from '@/composables/useFullscreen'

const { isFullscreen, toggleFullscreen } = useFullscreen()
</script>

<template><button class="icon-button" type="button" :aria-label="isFullscreen ? $t('shell.exitFullscreen') : $t('shell.enterFullscreen')" @click="toggleFullscreen"><svg v-if="isFullscreen" viewBox="0 0 24 24" aria-hidden="true"><path d="M8 3v3a2 2 0 0 1-2 2H3M16 3v3a2 2 0 0 0 2 2h3M8 21v-3a2 2 0 0 0-2-2H3M16 21v-3a2 2 0 0 1 2-2h3"/></svg><svg v-else viewBox="0 0 24 24" aria-hidden="true"><path d="M8 3H5a2 2 0 0 0-2 2v3M21 8V5a2 2 0 0 0-2-2h-3M3 16v3a2 2 0 0 0 2 2h3M16 21h3a2 2 0 0 0 2-2v-3"/></svg></button></template>

<style scoped>
.icon-button{align-items:center;background:transparent;border:0;border-radius:var(--radius-sm);color:var(--muted-foreground);cursor:pointer;display:inline-flex;justify-content:center;min-height:34px;padding:0;transition:background-color 140ms ease,color 140ms ease,scale 140ms ease;width:34px}.icon-button:hover{background:var(--surface-hover);color:var(--foreground)}.icon-button:active{scale:.96}.icon-button svg{fill:none;height:16px;stroke:currentColor;stroke-linecap:round;stroke-linejoin:round;stroke-width:1.7;width:16px}
</style>
