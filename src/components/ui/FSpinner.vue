<script setup lang="ts">
interface Props { size?: 'sm' | 'md'; label?: string }
const props = withDefaults(defineProps<Props>(), { size: 'md', label: 'Loading' })
</script>

<template>
  <span class="f-spinner" :class="`f-spinner-${props.size}`" role="status" :aria-label="props.label" />
</template>

<style scoped>
.f-spinner{animation:spin .7s linear infinite;border:2px solid currentColor;border-right-color:transparent;border-radius:50%;display:inline-block;flex:none}.f-spinner-sm{height:14px;width:14px}.f-spinner-md{height:20px;width:20px}@keyframes spin{to{transform:rotate(1turn)}}
</style>
