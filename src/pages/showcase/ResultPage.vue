<script setup lang="ts">
import FButton from '@/components/ui/FButton.vue'
import FPanel from '@/components/ui/FPanel.vue'
import FResult from '@/components/ui/FResult.vue'
</script>

<template><section class="page"><header><p class="eyebrow">{{ $t('showcase.eyebrow') }}</p><h1>{{ $t('showcase.resultsTitle') }}</h1><p>{{ $t('showcase.resultsDescription') }}</p></header><div class="result-grid"><FPanel><FResult tone="success" :title="$t('result.successTitle')" :description="$t('result.successDescription')"><RouterLink to="/projects"><FButton>{{ $t('result.viewProjects') }}</FButton></RouterLink></FResult></FPanel><FPanel><FResult tone="warning" :title="$t('result.warningTitle')" :description="$t('result.warningDescription')"><FButton variant="secondary">{{ $t('common.back') }}</FButton></FResult></FPanel><FPanel><FResult tone="error" :title="$t('result.errorTitle')" :description="$t('result.errorDescription')"><FButton variant="secondary">{{ $t('common.retry') }}</FButton></FResult></FPanel><FPanel><FResult tone="info" :title="$t('result.infoTitle')" :description="$t('result.infoDescription')" /></FPanel></div></section></template>

<style scoped>
.page{display:grid;gap:24px}.eyebrow{color:var(--primary);font-size:11px;font-weight:750;letter-spacing:.08em;margin:0 0 10px;text-transform:uppercase}.page h1{font-size:clamp(1.8rem,3vw,2.5rem);letter-spacing:-.045em;margin:0}.page>header>p:not(.eyebrow){color:var(--muted-foreground);font-size:14px;margin:10px 0 0}.result-grid{display:grid;gap:18px;grid-template-columns:repeat(2,minmax(0,1fr))}.result-grid :deep(.f-result){margin-inline:auto;min-height:260px;place-content:center}.result-grid a{text-decoration:none}@media(max-width:760px){.result-grid{grid-template-columns:1fr}}
</style>
