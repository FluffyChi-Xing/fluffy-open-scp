<script setup lang="ts">
interface Props { tone?: 'success' | 'warning' | 'error' | 'info'; title: string; description: string }
const props = withDefaults(defineProps<Props>(), { tone: 'info' })
</script>

<template><section class="f-result"><span class="f-result-icon" :class="`f-result-${props.tone}`" aria-hidden="true">{{ props.tone === 'success' ? '✓' : props.tone === 'error' ? '!' : props.tone === 'warning' ? '!' : 'i' }}</span><h1>{{ props.title }}</h1><p>{{ props.description }}</p><div class="f-result-actions"><slot /></div></section></template>

<style scoped>
.f-result{align-items:center;display:grid;justify-items:center;max-width:560px;text-align:center}.f-result-icon{align-items:center;border-radius:50%;display:flex;font-size:22px;font-weight:800;height:52px;justify-content:center;width:52px}.f-result-info{background:var(--accent);color:var(--primary)}.f-result-success{background:color-mix(in srgb,var(--success) 14%,transparent);color:var(--success)}.f-result-warning{background:color-mix(in srgb,var(--warning) 15%,transparent);color:var(--warning)}.f-result-error{background:color-mix(in srgb,var(--danger) 14%,transparent);color:var(--danger)}.f-result h1{font-size:24px;letter-spacing:-.035em;margin:16px 0 0}.f-result p{color:var(--muted-foreground);line-height:1.55;margin:9px 0 0}.f-result-actions{display:flex;flex-wrap:wrap;gap:10px;justify-content:center;margin-top:22px}
</style>
