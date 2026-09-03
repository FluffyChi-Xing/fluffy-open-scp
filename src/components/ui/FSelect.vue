<script setup lang="ts">
export interface SelectOption { label: string; value: string }
interface Props { id?: string; options: SelectOption[]; disabled?: boolean; invalid?: boolean }
const props = defineProps<Props>()
const model = defineModel<string>({ default: '' })
</script>

<template><select :id="props.id" v-model="model" class="f-select" :class="{ invalid: props.invalid }" :disabled="props.disabled" :aria-invalid="props.invalid || undefined"><option v-for="option in props.options" :key="option.value" :value="option.value">{{ option.label }}</option></select></template>

<style scoped>
.f-select{background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--foreground);min-height:38px;outline:0;padding:0 10px;width:100%}.f-select:hover:not(:disabled){border-color:var(--border-strong)}.f-select:focus{border-color:var(--primary);box-shadow:0 0 0 3px color-mix(in srgb,var(--ring) 25%,transparent)}.f-select.invalid{border-color:var(--danger)}.f-select:disabled{cursor:not-allowed;opacity:.6}
</style>
