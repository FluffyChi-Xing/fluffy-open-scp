<script setup lang="ts">
interface Props { id: string; label: string; required?: boolean; help?: string; error?: string }
const props = defineProps<Props>()
</script>

<template>
  <label class="f-form-field" :for="props.id">
    <span class="f-form-label">{{ props.label }}<b v-if="props.required" aria-hidden="true">*</b></span>
    <slot :id="props.id" :described-by="props.error ? `${props.id}-error` : props.help ? `${props.id}-help` : undefined" />
    <span v-if="props.error" :id="`${props.id}-error`" class="f-form-error">{{ props.error }}</span>
    <span v-else-if="props.help" :id="`${props.id}-help`" class="f-form-help">{{ props.help }}</span>
  </label>
</template>

<style scoped>
.f-form-field{display:grid;gap:7px}.f-form-label{color:var(--foreground);font-size:13px;font-weight:700}.f-form-label b{color:var(--danger);margin-inline-start:3px}.f-form-help,.f-form-error{font-size:12px;line-height:1.45}.f-form-help{color:var(--muted-foreground)}.f-form-error{color:var(--danger)}
</style>
