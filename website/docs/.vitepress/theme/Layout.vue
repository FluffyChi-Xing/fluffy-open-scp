<script setup lang="ts">
import { Content, useData } from 'vitepress'
import SiteHeader from './components/SiteHeader.vue'
import HomePage from './components/HomePage.vue'
import DocPage from './components/DocPage.vue'
import { useLocale } from './composables/useLocale'
const { frontmatter, page } = useData()
const { t, link } = useLocale()
</script>

<template>
  <a class="skip-link" href="#main">{{ t.skip }}</a>
  <SiteHeader :class="{ 'home-header': frontmatter.layout === 'home' }" />
  <main id="main" tabindex="-1">
    <section v-if="page.isNotFound" class="not-found wrap"><span class="eyebrow">404</span><h1>{{ t.notFound }}</h1><p>{{ t.notFoundBody }}</p><a class="button primary" :href="link()">{{ t.home }}</a></section>
    <HomePage v-else-if="frontmatter.layout === 'home'" />
    <DocPage v-else><Content /></DocPage>
  </main>
  <footer class="site-footer wrap">
    <div class="flex flex-wrap items-start justify-between gap-6"><a :href="link()" class="wordmark">OpenSCP<span class="brand-dot">.</span></a><p>{{ t.footer }}</p><a href="https://github.com/FluffyChi-Xing/fluffy-open-scp">GitHub ↗</a></div>
    <div class="footer-bottom"><span>{{ t.legal }}</span><span>MIT · OpenSCP</span></div>
  </footer>
</template>
