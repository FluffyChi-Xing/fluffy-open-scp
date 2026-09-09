<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import FSheet from "@/components/ui/FSheet.vue";
import {
  Carousel,
  CarouselContent,
  CarouselItem,
} from "@/components/ui/carousel";

interface PreviewSlide {
  id: string;
  screen: string;
  group: string;
  images: string[];
}

const { t } = useI18n();
const carousel = ref<InstanceType<typeof Carousel> | null>(null);
const slides = shallowRef<PreviewSlide[]>([]);
const editOpen = shallowRef(false);
const activeIndex = shallowRef(0);

const groupLabels = computed(() => {
  const keys = [
    "startup",
    "hud",
    "cityPanels",
    "region",
    "meta",
    "asp",
    "ep1",
  ];
  return Object.fromEntries(
    keys.map((key) => [key, t(`studio.ui.groups.${key}`)]),
  );
});

onMounted(async () => {
  try {
    const response = await fetch("/game-ui/ui-preview.json");
    const data = (await response.json()) as { slides: PreviewSlide[] };
    slides.value = data.slides;
  } catch {
    slides.value = [];
  }
});

function prev() {
  carousel.value?.scrollPrev();
}
function next() {
  carousel.value?.scrollNext();
}
function onSlideChange(index: number) {
  activeIndex.value = index;
}
function openEditor() {
  editOpen.value = true;
}
</script>

<template>
  <section class="ui-page">
    <RouterLink class="back-link" to="/studio">
      <FIcon name="ArrowLeft" :size="14" />
      {{ t("studio.backToStudio") }}
    </RouterLink>

    <header class="page-header">
      <span class="page-icon"><FIcon name="PanelTop" :size="20" /></span>
      <div>
        <FTypography :header="2" spacing="none">{{
          t("studio.panels.ui.title")
        }}</FTypography>
        <p class="page-meta">{{ t("studio.panels.ui.meta") }}</p>
      </div>
      <div class="toolbar" role="toolbar" :aria-label="t('studio.ui.toolbar')">
        <button
          type="button"
          class="tool-button"
          :title="t('studio.ui.prev')"
          :aria-label="t('studio.ui.prev')"
          @click="prev()"
        >
          <FIcon name="ChevronLeft" :size="16" />
        </button>
        <button
          type="button"
          class="tool-button"
          :title="t('studio.ui.next')"
          :aria-label="t('studio.ui.next')"
          @click="next()"
        >
          <FIcon name="ChevronRight" :size="16" />
        </button>
        <span class="slide-counter mono">
          {{ activeIndex + 1 }} / {{ slides.length }}
        </span>
        <button type="button" class="edit-button" @click="openEditor">
          <FIcon name="Pencil" :size="14" />
          {{ t("studio.ui.edit") }}
        </button>
      </div>
    </header>

    <p v-if="!slides.length" class="notice" role="status">
      {{ t("studio.ui.noSlides") }}
    </p>

    <Carousel
      v-else
      ref="carousel"
      :options="{ loop: true }"
      class="preview-carousel"
      @select="onSlideChange"
    >
      <CarouselContent>
        <CarouselItem v-for="slide in slides" :key="slide.id">
          <article class="slide">
            <div class="slide-head">
              <span class="group-chip">{{
                groupLabels[slide.group] ?? slide.group
              }}</span>
              <h3 class="slide-title mono">{{ slide.id }}</h3>
            </div>
            <div class="slide-stage">
              <div class="stage-frame">
                <img
                  v-for="(image, index) in slide.images"
                  :key="index"
                  :src="image"
                  :alt="`${slide.id} asset ${index + 1}`"
                  loading="lazy"
                  class="stage-image"
                  :style="{ zIndex: slide.images.length - index }"
                />
                <span class="stage-watermark">{{
                  t("studio.ui.rebuildNotice")
                }}</span>
              </div>
            </div>
            <p class="slide-caption">{{ t("studio.ui.caption") }}</p>
          </article>
        </CarouselItem>
      </CarouselContent>
    </Carousel>

    <FSheet v-model:open="editOpen" :label="t('studio.ui.editSheetLabel')" width="min(480px,92vw)">
      <div class="sheet-inner">
        <header class="sheet-header">
          <FTypography :header="4" spacing="none">{{
            t("studio.ui.editSheetTitle", slides[activeIndex]?.id ?? "")
          }}</FTypography>
          <p class="sheet-meta mono">{{ slides[activeIndex]?.screen }}</p>
        </header>
        <FEmpty
          :description="t('studio.ui.editPlaceholder')"
          icon-name="PanelTop"
        />
      </div>
    </FSheet>
  </section>
</template>

<style scoped>
.ui-page {
  padding-bottom: 3rem;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}
.back-link {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.75rem;
  color: var(--muted-foreground);
  text-decoration: none;
  width: fit-content;
}
.back-link:hover {
  color: var(--foreground);
}
.page-header {
  display: flex;
  align-items: center;
  gap: 1rem;
  flex-wrap: wrap;
}
.page-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--accent);
  flex-shrink: 0;
}
.page-header :deep(h2) {
  margin: 0;
}
.page-meta {
  margin: 0.2rem 0 0;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
}
.toolbar {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
.tool-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--muted-foreground);
  cursor: pointer;
  transition: color 140ms ease, border-color 140ms ease;
}
.tool-button:hover:not(:disabled) {
  color: var(--foreground);
  border-color: var(--accent);
}
.tool-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.slide-counter {
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
  min-width: 3.5rem;
  text-align: center;
}
.edit-button {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.4rem 0.8rem;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: var(--primary);
  color: var(--primary-foreground);
  font-size: 0.75rem;
  cursor: pointer;
}
.edit-button:hover {
  filter: brightness(1.08);
}
.mono {
  font-family: var(--font-mono, ui-monospace, monospace);
}
.notice {
  margin: 0;
  padding: 0.7rem 1rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--muted-foreground);
  font-size: 0.8125rem;
}
.preview-carousel {
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  overflow: hidden;
}
.slide {
  padding: 1.5rem 1.75rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.slide-head {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-wrap: wrap;
}
.group-chip {
  font-size: 0.6875rem;
  padding: 0.15rem 0.6rem;
  border-radius: 999px;
  border: 1px solid color-mix(in oklab, var(--accent) 45%, transparent);
  color: var(--accent);
  white-space: nowrap;
}
.slide-title {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
}
.slide-stage {
  display: flex;
  justify-content: center;
}
.stage-frame {
  position: relative;
  width: 100%;
  max-width: 720px;
  aspect-ratio: 16 / 9;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background:
    repeating-conic-gradient(
      color-mix(in oklab, var(--border) 30%, transparent) 0% 25%,
      transparent 0% 50%
    )
    0 0 / 24px 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}
.stage-image {
  position: absolute;
  height: 82%;
  max-width: 90%;
  object-fit: contain;
  border: 1px solid color-mix(in oklab, var(--foreground) 15%, transparent);
  border-radius: var(--radius-sm);
  background: var(--background);
  box-shadow: var(--shadow-sm);
}
.stage-image:nth-child(2) {
  translate: -12% -8%;
  rotate: -4deg;
}
.stage-image:nth-child(3) {
  translate: 12% 8%;
  rotate: 3deg;
}
.stage-watermark {
  position: absolute;
  bottom: 0.6rem;
  right: 0.75rem;
  font-size: 0.625rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--subtle-foreground);
  background: color-mix(in oklab, var(--surface) 80%, transparent);
  padding: 0.15rem 0.5rem;
  border-radius: 999px;
}
.slide-caption {
  margin: 0;
  font-size: 0.75rem;
  color: var(--subtle-foreground);
  text-align: center;
}
.sheet-inner {
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  min-height: 60vh;
}
.sheet-header :deep(h4) {
  margin: 0;
}
.sheet-meta {
  margin: 0.25rem 0 0;
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
}
</style>
