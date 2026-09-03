<script setup lang="ts">
interface Props { width?: string; height?: string; rounded?: boolean }
const props = withDefaults(defineProps<Props>(), { width: '100%', height: '1em' })
</script>

<template><span class="f-skeleton" :class="{ rounded: props.rounded }" :style="{ width: props.width, height: props.height }" /></template>

<style scoped>
.f-skeleton{animation:pulse 1.45s ease-in-out infinite;background:linear-gradient(90deg,var(--surface-hover),var(--border),var(--surface-hover));background-size:200% 100%;border-radius:5px;display:block}.f-skeleton.rounded{border-radius:999px}@keyframes pulse{50%{background-position:-100% 0}}
</style>
