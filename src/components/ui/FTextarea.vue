<script setup lang="ts">
interface Props { id?: string; placeholder?: string; disabled?: boolean; invalid?: boolean; rows?: number }
const props = withDefaults(defineProps<Props>(), { rows: 4 })
const model = defineModel<string>({ default: '' })
</script>

<template><textarea :id="props.id" v-model="model" class="f-textarea" :class="{ invalid: props.invalid }" :placeholder="props.placeholder" :disabled="props.disabled" :rows="props.rows" :aria-invalid="props.invalid || undefined" /></template>

<style scoped>
.f-textarea{background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--foreground);outline:0;padding:9px 10px;resize:vertical;width:100%}.f-textarea:hover:not(:disabled){border-color:var(--border-strong)}.f-textarea:focus{border-color:var(--primary);box-shadow:0 0 0 3px color-mix(in srgb,var(--ring) 25%,transparent)}.f-textarea.invalid{border-color:var(--danger)}.f-textarea:disabled{cursor:not-allowed;opacity:.6}
</style>
