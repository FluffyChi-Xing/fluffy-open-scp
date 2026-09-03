<script setup lang="ts">
interface Props { id?: string; disabled?: boolean }
const props = defineProps<Props>()
const model = defineModel<boolean>({ default: false })
</script>

<template><input :id="props.id" v-model="model" class="f-checkbox" type="checkbox" :disabled="props.disabled" /></template>

<style scoped>
.f-checkbox{accent-color:var(--primary);block-size:18px;cursor:pointer;inline-size:18px}.f-checkbox:disabled{cursor:not-allowed;opacity:.6}
</style>
