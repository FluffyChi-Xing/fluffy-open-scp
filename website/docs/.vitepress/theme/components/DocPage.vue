<script setup lang="ts">
import { useData, useRoute } from 'vitepress'
import ResearchSection from './ResearchSection.vue'
import { useLocale } from '../composables/useLocale'
const { frontmatter, page } = useData()
const route = useRoute()
const { t, link } = useLocale()
const paths = ['about', 'guide/getting-started', 'guide/workbench', 'development', 'research/']
</script>
<template>
  <ResearchSection v-if="frontmatter.layout === 'archive'" archive />
  <div v-else class="doc-layout wrap"><aside class="doc-sidebar"><p class="eyebrow">{{ t.docsTitle }}</p><nav :aria-label="t.docsTitle"><a v-for="(path, index) in paths" :key="path" :href="link(path)" :aria-current="route.path === link(path) ? 'page' : undefined">{{ [t.nav[0], t.nav[1], t.workEyebrow, t.nav[2], t.nav[3]][index] }}</a></nav><div class="sidebar-note">{{ t.updated }}</div></aside><article class="doc-article"><a v-if="frontmatter.research" class="text-link article-back" :href="link('research/')">← {{ t.articleBack }}</a><div class="prose"><slot /></div><div class="doc-bottom"><a :href="link('guide/getting-started')">{{ t.start }} →</a><a href="https://github.com/FluffyChi-Xing/fluffy-open-scp">{{ t.source }} ↗</a></div></article><aside v-if="page.headers.length" class="doc-toc"><p>{{ t.onPage }}</p><a v-for="header in page.headers" :key="header.slug" :href="`#${header.slug}`">{{ header.title }}</a></aside></div>
</template>
