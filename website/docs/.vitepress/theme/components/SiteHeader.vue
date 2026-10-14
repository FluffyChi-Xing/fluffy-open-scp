<script setup lang="ts">
import { shallowRef, watch } from 'vue'
import { useRoute, useData, withBase } from 'vitepress'
import { Menu, X, Languages, ArrowUpRight, Sun, Moon } from 'lucide-vue-next'
import { useLocale } from '../composables/useLocale'
const { t, link, alternate, english } = useLocale()
const route = useRoute()
const { isDark } = useData()
const open = shallowRef(false)
const paths = ['about', 'guide/getting-started', 'development', 'research/']
watch(() => route.path, () => { open.value = false })
</script>

<template>
  <header class="site-header" @keydown.esc="open = false">
    <div class="header-inner wrap">
      <a class="brand" :href="link()" :aria-label="`OpenSCP · ${t.home}`"><img :src="withBase('/images/logo.png')" alt="" width="42" height="42"><span class="wordmark">OpenSCP<span class="brand-dot">.</span></span></a>
      <nav class="desktop-nav" :aria-label="t.menu"><a v-for="(label, i) in t.nav" :key="paths[i]" :href="link(paths[i])" :aria-current="route.path === link(paths[i]) ? 'page' : undefined">{{ label }}</a></nav>
      <div class="header-actions"><button class="theme-toggle" type="button" role="switch" :aria-checked="isDark" :aria-label="t.darkMode" :title="isDark ? t.lightModeAction : t.darkModeAction" @click="isDark = !isDark"><Sun v-if="isDark" :size="18" aria-hidden="true"/><Moon v-else :size="18" aria-hidden="true"/></button><a class="language-button" :href="alternate" :aria-label="t.language" :hreflang="english ? 'zh-CN' : 'en-US'"><Languages :size="17"/><span>{{ english ? '中文' : 'EN' }}</span></a><a class="github-link" href="https://github.com/FluffyChi-Xing/fluffy-open-scp">GitHub <ArrowUpRight :size="15"/></a><button class="mobile-toggle" :aria-label="open ? t.close : t.menu" :aria-expanded="open" aria-controls="mobile-nav" @click="open = !open"><X v-if="open" :size="22"/><Menu v-else :size="22"/></button></div>
    </div>
    <nav v-if="open" id="mobile-nav" class="mobile-nav" :aria-label="t.menu"><a v-for="(label, i) in t.nav" :key="paths[i]" :href="link(paths[i])">{{ label }}</a></nav>
  </header>
</template>
