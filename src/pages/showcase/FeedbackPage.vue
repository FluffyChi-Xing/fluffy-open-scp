<script setup lang="ts">
import { shallowRef } from 'vue'
import FButton from '@/components/ui/FButton.vue'
import FPanel from '@/components/ui/FPanel.vue'
import FSkeleton from '@/components/ui/FSkeleton.vue'
import FSpinner from '@/components/ui/FSpinner.vue'
import { useLoading } from '@/composables/useLoading'
import { useToast } from '@/composables/useToast'

const { success, info, warning, error } = useToast()
const { loading, run } = useLoading()
const showSkeleton = shallowRef(true)
async function simulateLoad() { await run(() => new Promise<void>((resolve) => window.setTimeout(resolve, 900))) }
</script>

<template><section class="page"><header><p class="eyebrow">{{ $t('showcase.eyebrow') }}</p><h1>{{ $t('showcase.feedbackTitle') }}</h1><p>{{ $t('showcase.feedbackDescription') }}</p></header><div class="feedback-grid"><FPanel><h2>{{ $t('feedback.toast') }}</h2><div class="button-grid"><FButton @click="success($t('toast.success'))">{{ $t('feedback.success') }}</FButton><FButton variant="secondary" @click="info($t('toast.info'))">{{ $t('feedback.info') }}</FButton><FButton variant="secondary" @click="warning($t('toast.warning'))">{{ $t('feedback.warning') }}</FButton><FButton variant="danger" @click="error($t('toast.error'))">{{ $t('feedback.error') }}</FButton></div></FPanel><FPanel><h2>{{ $t('feedback.loading') }}</h2><div class="loading-demo"><FSpinner v-if="loading" :label="$t('common.loading')" /><span>{{ loading ? $t('common.loading') : $t('common.ready') }}</span><FButton variant="secondary" :loading="loading" @click="simulateLoad">{{ $t('common.retry') }}</FButton></div></FPanel><FPanel><div class="panel-heading"><h2>{{ $t('feedback.skeleton') }}</h2><FButton variant="ghost" @click="showSkeleton=!showSkeleton">{{ $t('feedback.toggle') }}</FButton></div><div v-if="showSkeleton" class="skeleton-stack"><FSkeleton width="42%" height="12px" /><FSkeleton height="16px" /><FSkeleton width="76%" height="16px" /><FSkeleton width="110px" height="34px" /></div><p v-else class="loaded-copy">{{ $t('feedback.loaded') }}</p></FPanel></div></section></template>

<style scoped>
.page{display:grid;gap:24px}.eyebrow{color:var(--primary);font-size:11px;font-weight:750;letter-spacing:.08em;margin:0 0 10px;text-transform:uppercase}.page h1{font-size:clamp(1.8rem,3vw,2.5rem);letter-spacing:-.045em;margin:0}.page>header>p:not(.eyebrow){color:var(--muted-foreground);font-size:14px;margin:10px 0 0}.feedback-grid{display:grid;gap:18px;grid-template-columns:repeat(3,minmax(0,1fr))}.feedback-grid h2{font-size:14px;margin:0 0 15px}.button-grid{display:grid;gap:8px;grid-template-columns:repeat(2,minmax(0,1fr))}.loading-demo{align-items:center;display:flex;flex-wrap:wrap;gap:10px}.loading-demo span{color:var(--muted-foreground);font-size:13px}.panel-heading{align-items:center;display:flex;justify-content:space-between}.skeleton-stack{display:grid;gap:11px}.loaded-copy{color:var(--muted-foreground);font-size:13px;line-height:1.6;margin:0}@media(max-width:900px){.feedback-grid{grid-template-columns:1fr}}
</style>
