<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import FButton from '@/components/ui/FButton.vue'

const route = useRoute()
const router = useRouter()
const canGoBack = computed(() => window.history.length > 1)
</script>

<template><main class="not-found"><div class="glow glow-a" aria-hidden="true" /><div class="glow glow-b" aria-hidden="true" /><article class="not-found-card"><p class="hero" aria-hidden="true"><span class="digit">4</span><span class="digit digit-accent">0</span><span class="digit">4</span></p><h1>{{ $t('notFound.title') }}</h1><p class="description">{{ $t('notFound.description') }}</p><div class="missing-path"><span class="label">{{ $t('notFound.path') }}</span><code>{{ route.fullPath }}</code></div><div class="actions"><RouterLink to="/" class="link"><FButton>{{ $t('notFound.action') }}</FButton></RouterLink><FButton v-if="canGoBack" variant="secondary" @click="router.back()">{{ $t('notFound.back') }}</FButton></div></article></main></template>

<style scoped>
.not-found{align-items:center;display:flex;justify-content:center;min-height:100vh;overflow:hidden;padding:24px;position:relative}.glow{border-radius:50%;filter:blur(72px);opacity:.45;pointer-events:none;position:absolute}.glow-a{background:color-mix(in srgb,var(--brand) 34%,transparent);height:340px;inset-inline-start:-120px;top:-80px;width:340px}.glow-b{background:color-mix(in srgb,var(--accent) 75%,transparent);bottom:-120px;height:300px;inset-inline-end:-100px;width:300px}.not-found-card{align-items:center;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow-md);display:flex;flex-direction:column;max-width:560px;padding:clamp(30px,6vw,56px);position:relative;text-align:center;width:100%}.hero{font-size:clamp(4.25rem,13vw,6.5rem);font-weight:800;letter-spacing:-.05em;line-height:1;margin:0}.digit{display:inline-block}.digit-accent{background:linear-gradient(135deg,var(--primary),color-mix(in srgb,var(--primary) 55%,white));background-clip:text;color:transparent}h1{font-size:clamp(1.35rem,3vw,1.7rem);letter-spacing:-.035em;margin:22px 0 0}.description{color:var(--muted-foreground);font-size:14px;line-height:1.6;margin:10px 0 0;max-width:42ch}.missing-path{background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-md);display:flex;flex-direction:column;gap:6px;margin-top:26px;padding:12px 14px;text-align:start;width:100%}.missing-path .label{color:var(--subtle-foreground);font-size:10px;font-weight:700;letter-spacing:.08em;text-transform:uppercase}.missing-path code{color:var(--foreground);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;line-height:1.5;overflow-wrap:anywhere}.actions{align-items:center;display:flex;flex-wrap:wrap;gap:10px;justify-content:center;margin-top:26px}.link{text-decoration:none}
</style>
