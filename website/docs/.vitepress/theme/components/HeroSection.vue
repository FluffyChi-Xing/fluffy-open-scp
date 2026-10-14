<script setup lang="ts">
import { ArrowRight, ArrowUpRight, Box, Layers3, ScanLine, Network, MessageCircle } from 'lucide-vue-next'
import ShaderTerrain from './ShaderTerrain.vue'
import { useLocale } from '../composables/useLocale'
const { t, link } = useLocale()
const icons = [Box, ScanLine, Layers3, Network]
</script>
<template>
  <section class="hero">
    <ShaderTerrain />
    <div class="hero-grid wrap">
      <div class="hero-copy"><p class="hero-tag"><span class="status-dot"/>{{ t.tag }}</p><h1>{{ t.title }}<br><span>{{ t.titleAccent }}</span></h1><p class="hero-intro">{{ t.intro }}</p><div class="flex flex-wrap gap-3"><a class="button primary" :href="link('guide/getting-started')">{{ t.start }}<ArrowRight :size="18"/></a><a class="button secondary" href="https://github.com/FluffyChi-Xing/fluffy-open-scp">{{ t.source }}<ArrowUpRight :size="17"/></a><a class="button secondary" href="https://discord.gg/VuRDH8px2" target="_blank" rel="noopener noreferrer"><MessageCircle :size="18" aria-hidden="true"/>{{ t.discord }}<ArrowUpRight :size="17" aria-hidden="true"/></a></div><p class="hero-note">{{ t.heroNote }}</p></div>
    </div>
  </section>
    <div class="capability-strip wrap"><span v-for="(text, index) in t.strip" :key="text"><component :is="icons[index]" :size="18"/>{{ text }}</span></div>
</template>
