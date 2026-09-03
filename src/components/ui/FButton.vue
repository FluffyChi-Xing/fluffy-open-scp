<script setup lang="ts">
import FSpinner from '@/components/ui/FSpinner.vue'

interface Props {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
  loading?: boolean
  disabled?: boolean
  type?: 'button' | 'submit' | 'reset'
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'primary',
  type: 'button'
})
</script>

<template>
  <button class="f-button" :class="`f-button-${props.variant}`" :type="props.type" :disabled="props.disabled || props.loading">
    <FSpinner v-if="props.loading" size="sm" />
    <slot />
  </button>
</template>

<style scoped>
.f-button{align-items:center;border:1px solid transparent;border-radius:var(--radius-md);cursor:pointer;display:inline-flex;font-size:13px;font-weight:700;gap:8px;justify-content:center;min-height:38px;padding:0 13px;transition:background-color 140ms ease,border-color 140ms ease,color 140ms ease,scale 140ms ease}.f-button:active:not(:disabled){scale:.96}.f-button:disabled{cursor:not-allowed;opacity:.55}.f-button-primary{background:var(--primary);color:var(--primary-foreground)}.f-button-primary:hover:not(:disabled){background:var(--primary-hover)}.f-button-secondary{background:var(--surface);border-color:var(--border);color:var(--foreground)}.f-button-secondary:hover:not(:disabled){background:var(--surface-hover)}.f-button-ghost{background:transparent;color:var(--muted-foreground)}.f-button-ghost:hover:not(:disabled){background:var(--surface-hover);color:var(--foreground)}.f-button-danger{background:var(--danger);color:var(--primary-foreground)}
</style>
