<script setup lang="ts">
import { ArrowUpRight, Network, Layers3, FileBox } from 'lucide-vue-next'
import { useLocale } from '../composables/useLocale'
defineProps<{ archive?: boolean }>()
const { t, link } = useLocale()
const icons = [Network, Layers3, FileBox]
</script>
<template>
  <section :class="['research-section', { 'archive-section': archive }]"><div class="wrap"><div class="section-heading"><div><p class="eyebrow">{{ t.researchEyebrow }}</p><component :is="archive ? 'h1' : 'h2'">{{ t.researchTitle }}</component><p class="research-intro">{{ t.researchIntro }}</p></div><a v-if="!archive" class="text-link" :href="link('research/')">{{ t.allResearch }}<ArrowUpRight :size="17"/></a></div>
    <div class="research-grid"><a v-for="(article, index) in t.articles" :key="article.slug" class="research-card" :href="link(`research/${article.slug}`)"><div :class="['research-art', `art-${index}`]" aria-hidden="true"><component :is="icons[index]" :size="62" :stroke-width="1"/><span class="art-code">{{ ['GB::GameManager', 'Property → Raster → GPU', 'Type / Group / Instance'][index] }}</span></div><div class="research-card-body"><p class="eyebrow">{{ article.category }}</p><h3>{{ article.title }}<ArrowUpRight :size="20"/></h3><p>{{ article.description }}</p><div class="article-meta">{{ article.meta }}<span aria-hidden="true">↗</span></div></div></a></div>
  </div></section>
</template>
