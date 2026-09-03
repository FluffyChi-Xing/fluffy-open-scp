<script setup lang="ts">
interface Props { id?: string; placeholder?: string; disabled?: boolean; invalid?: boolean; type?: 'email' | 'password' | 'text' }
const props = withDefaults(defineProps<Props>(), { type: 'text' })
const model = defineModel<string>({ default: '' })
</script>

<template><input :id="props.id" v-model="model" class="f-input" :class="{ invalid: props.invalid }" :type="props.type" :placeholder="props.placeholder" :disabled="props.disabled" :aria-invalid="props.invalid || undefined" /></template>

<style scoped>
.f-input{background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--foreground);min-height:38px;outline:0;padding:0 10px;width:100%}.f-input:hover:not(:disabled){border-color:var(--border-strong)}.f-input:focus{border-color:var(--primary);box-shadow:0 0 0 3px color-mix(in srgb,var(--ring) 25%,transparent)}.f-input.invalid{border-color:var(--danger)}.f-input:disabled{cursor:not-allowed;opacity:.6}
</style>
