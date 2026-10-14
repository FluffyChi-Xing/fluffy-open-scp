<script setup lang="ts">
import { onMounted, onUnmounted, useTemplateRef } from 'vue'
import { mountTerrain } from '../composables/terrain'
const canvas = useTemplateRef<HTMLCanvasElement>('canvas')
let dispose: (() => void) | undefined
onMounted(() => { if (canvas.value) dispose = mountTerrain(canvas.value) })
onUnmounted(() => dispose?.())
</script>
<template><div class="shader-terrain" aria-hidden="true"><canvas ref="canvas" /></div></template>
