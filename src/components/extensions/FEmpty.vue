<script setup lang="ts">
import { computed } from 'vue'
import FIcon from './FIcon.vue'

type EmptyVariant = 'default' | 'compact'
type EmptyStatus = 'default' | 'info' | 'success' | 'warning' | 'error'

interface Props {
  iconName?: string
  title?: string
  desc?: string
  variant?: EmptyVariant
  status?: EmptyStatus
}

const props = withDefaults(defineProps<Props>(), {
  iconName: 'Box',
  variant: 'default',
  status: 'info',
})

const classes = computed(() => [
  'f-empty',
  `f-empty-${props.variant}`,
  `f-empty-${props.status}`,
])
</script>

<template>
  <section :class="classes">
    <div class="f-empty-icon"><FIcon :name="props.iconName" :size="props.variant === 'compact' ? 28 : 36" /></div>
    <h2 v-if="props.title" class="f-empty-title">{{ props.title }}</h2>
    <p v-if="props.desc" class="f-empty-desc">{{ props.desc }}</p>
    <div v-if="$slots.default" class="f-empty-actions"><slot /></div>
  </section>
</template>

<style scoped>
.f-empty{align-items:center;display:grid;justify-items:center;max-width:480px;padding:40px 24px;text-align:center}.f-empty-icon{align-items:center;background:var(--accent);border-radius:50%;color:var(--primary);display:flex;height:72px;justify-content:center;width:72px}.f-empty-title{font-size:20px;letter-spacing:-.03em;line-height:1.3;margin:18px 0 0}.f-empty-desc{color:var(--muted-foreground);line-height:1.55;margin:8px 0 0}.f-empty-actions{display:flex;flex-wrap:wrap;gap:10px;justify-content:center;margin-top:20px}.f-empty-default .f-empty-icon{background:var(--surface-hover);color:var(--muted-foreground)}.f-empty-info .f-empty-icon{background:var(--accent);color:var(--primary)}.f-empty-success .f-empty-icon{background:color-mix(in srgb,var(--success) 14%,transparent);color:var(--success)}.f-empty-warning .f-empty-icon{background:color-mix(in srgb,var(--warning) 15%,transparent);color:var(--warning)}.f-empty-error .f-empty-icon{background:color-mix(in srgb,var(--danger) 14%,transparent);color:var(--danger)}.f-empty-compact{padding:20px 16px}.f-empty-compact .f-empty-icon{height:52px;width:52px}.f-empty-compact .f-empty-title{font-size:16px;margin-top:12px}.f-empty-compact .f-empty-desc{font-size:13px;margin-top:5px}.f-empty-compact .f-empty-actions{margin-top:14px}
</style>
