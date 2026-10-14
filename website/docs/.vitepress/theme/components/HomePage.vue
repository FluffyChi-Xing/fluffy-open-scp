<script setup lang="ts">
import HeroSection from './HeroSection.vue'
import WorkbenchSection from './WorkbenchSection.vue'
import ResearchSection from './ResearchSection.vue'
import ProgressSection from './ProgressSection.vue'
import { useLocale } from '../composables/useLocale'
const { t, link } = useLocale()
</script>
<template>
  <HeroSection />
  <WorkbenchSection />
  <ResearchSection />
  <ProgressSection />
  <section class="closing wrap"><div><h2>{{ t.ctaTitle }}</h2><p>{{ t.ctaBody }}</p></div><a class="button primary" :href="link('guide/getting-started')">{{ t.start }} <span aria-hidden="true">→</span></a></section>
</template>
