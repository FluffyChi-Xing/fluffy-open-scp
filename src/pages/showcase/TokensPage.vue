<script setup lang="ts">
import FPanel from '@/components/ui/FPanel.vue'

const colors = ['--background', '--surface', '--surface-elevated', '--surface-hover', '--foreground', '--muted-foreground', '--primary', '--success', '--warning', '--danger']
const radii = ['--radius-sm', '--radius-md', '--radius-lg']
const shadows = ['--shadow-sm', '--shadow-md']
</script>

<template><section class="page"><header><p class="eyebrow">{{ $t('showcase.eyebrow') }}</p><h1>{{ $t('showcase.tokensTitle') }}</h1><p>{{ $t('showcase.tokensDescription') }}</p></header><FPanel><h2>{{ $t('tokens.colors') }}</h2><div class="color-grid"><div v-for="token in colors" :key="token" class="color-token"><span :style="{ background: `var(${token})` }" /><code>{{ token }}</code></div></div></FPanel><div class="token-grid"><FPanel><h2>{{ $t('tokens.radius') }}</h2><div class="shape-stack"><div v-for="token in radii" :key="token" :style="{ borderRadius: `var(${token})` }"><code>{{ token }}</code></div></div></FPanel><FPanel><h2>{{ $t('tokens.shadows') }}</h2><div class="shadow-stack"><div v-for="token in shadows" :key="token" :style="{ boxShadow: `var(${token})` }"><code>{{ token }}</code></div></div></FPanel><FPanel><h2>{{ $t('tokens.focus') }}</h2><button class="focus-demo" type="button">:focus-visible</button></FPanel></div></section></template>

<style scoped>
.page{display:grid;gap:24px}.eyebrow{color:var(--primary);font-size:11px;font-weight:750;letter-spacing:.08em;margin:0 0 10px;text-transform:uppercase}.page h1{font-size:clamp(1.8rem,3vw,2.5rem);letter-spacing:-.045em;margin:0}.page>header>p:not(.eyebrow){color:var(--muted-foreground);font-size:14px;margin:10px 0 0}.page h2{font-size:14px;margin:0 0 16px}.color-grid{display:grid;gap:12px;grid-template-columns:repeat(auto-fit,minmax(160px,1fr))}.color-token{display:grid;gap:7px}.color-token span{border:1px solid var(--border);border-radius:var(--radius-sm);height:64px}.color-token code,.shape-stack code,.shadow-stack code{color:var(--muted-foreground);font-size:11px}.token-grid{display:grid;gap:18px;grid-template-columns:repeat(3,minmax(0,1fr))}.shape-stack,.shadow-stack{display:grid;gap:12px}.shape-stack>div,.shadow-stack>div{align-items:center;background:var(--surface-elevated);border:1px solid var(--border);display:flex;height:56px;justify-content:center;padding:8px}.focus-demo{background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--foreground);cursor:pointer;font-size:13px;padding:10px 13px}@media(max-width:860px){.token-grid{grid-template-columns:1fr}}
</style>
