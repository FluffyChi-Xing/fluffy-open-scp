<script setup lang="ts">
import { shallowRef } from 'vue'
import { withBase } from 'vitepress'
import { ArrowRight } from 'lucide-vue-next'
import { useLocale } from '../composables/useLocale'
const { t, link, english } = useLocale()
const selected = shallowRef(0)
const images = ['source-parsing.png', 'rendering.png', 'map-review.png', 'raster-paint.png', 'resource-overview.png']
function navigate(event: KeyboardEvent) {
  const keys = ['ArrowRight', 'ArrowLeft', 'Home', 'End']
  if (!keys.includes(event.key)) return
  event.preventDefault()
  selected.value = event.key === 'Home' ? 0 : event.key === 'End' ? images.length - 1 : (selected.value + (event.key === 'ArrowRight' ? 1 : images.length - 1)) % images.length
  document.getElementById(`work-tab-${selected.value}`)?.focus()
}
</script>
<template>
  <section class="section wrap" id="workbench"><div class="section-heading"><div><p class="eyebrow">{{ t.workEyebrow }}</p><h2>{{ t.workTitle }}</h2></div><p class="section-description">{{ t.workIntro }}</p></div>
    <div class="workbench"><div class="work-tabs" role="tablist" :aria-label="t.workTitle" @keydown="navigate"><button v-for="(tab, index) in t.tabs" :key="index" :id="`work-tab-${index}`" role="tab" :aria-selected="selected === index" :aria-controls="`work-panel-${index}`" :tabindex="selected === index ? 0 : -1" @click="selected = index"><span class="tab-index">0{{ index + 1 }}</span>{{ tab }}</button></div>
      <div :id="`work-panel-${selected}`" class="work-panel" role="tabpanel" :aria-labelledby="`work-tab-${selected}`" tabindex="0"><div class="work-copy"><h3>{{ t.tabTitles[selected] }}</h3><p>{{ t.tabDescriptions[selected] }}</p><a class="text-link" :href="link('guide/workbench')">{{ t.guideLink }} <ArrowRight :size="16"/></a></div><div class="work-image"><img :src="withBase(`/images/${english ? 'en-US' : 'zh-CN'}/${images[selected]}`)" :alt="t.tabs[selected]" width="1920" height="1129" loading="lazy"></div></div>
    </div><p class="image-note">{{ t.screenshotNote }}</p>
  </section>
</template>
