<script setup lang="ts">
import { useLocale } from '../composables/useLocale'
const { t, link } = useLocale()
</script>
<template>
  <section class="section wrap"><div class="section-heading"><div><p class="eyebrow">{{ t.progressEyebrow }}</p><h2>{{ t.progressTitle }}</h2><p class="research-intro">{{ t.progressIntro }}</p></div><a class="text-link" :href="link('development')">{{ t.roadmapLink }} <span aria-hidden="true">→</span></a></div><div class="progress-grid"><article v-for="(phase, index) in t.phases" :key="phase.title" class="progress-item"><div class="phase-line"><span :class="['phase-dot', `phase-${index}`]"/><span class="phase-state">{{ phase.state }}</span></div><h3>{{ phase.title }}</h3><p>{{ phase.body }}</p></article></div><p class="image-note">{{ t.updated }}</p></section>
</template>
