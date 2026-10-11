<script setup lang="ts">
import { useI18n } from "vue-i18n";
import RasterPreviewCard from "./RasterPreviewCard.vue";
import RasterLotPreview from "./RasterLotPreview.vue";
import type { RasterSource } from "@/lib/raster-editor/schema";
import { computed } from "vue";
import type { RasterDocument } from "@/lib/raster-editor/document";
import {
  rgbaBase64ToPngDataUrl,
  rgbaToBase64,
  simulateLotDataUrl,
} from "@/lib/raster-editor/encoders";
import {
  channelPixels,
  type Channel,
  type RasterProfile,
} from "@/lib/raster-editor/schema";
const props = defineProps<{
  target: RasterSource | null;
  metersPerPixel: number;
  doc: RasterDocument;
  version: number;
  channel: Channel;
  profile: RasterProfile;
  colors: [number, number, number, number][];
}>();
const { t } = useI18n();
const raw = computed(() => {
  void props.version;
  return rgbaToBase64(props.doc.pixels);
});
const rawPng = computed(() =>
  rgbaBase64ToPngDataUrl(raw.value, props.doc.width, props.doc.height),
);
const preview = computed(() => {
  void props.version;
  return rgbaBase64ToPngDataUrl(
    rgbaToBase64(channelPixels(props.doc.pixels, props.channel)),
    props.doc.width,
    props.doc.height,
  );
});
const materialPreview = computed(() => {
  void props.version;
  return props.profile === "lot-mask"
    ? simulateLotDataUrl(
        props.doc,
        null,
        props.colors.map((c) => [c[0], c[1], c[2]] as const),
      )
    : "";
});
</script>
<template>
  <section class="preview-panel">
    <RasterPreviewCard
      :title="
        channel === 'rgba'
          ? t('rasterEditor.raw')
          : t('rasterEditor.channel', { channel: channel.toUpperCase() })
      "
    >
      <img :src="preview" :alt="t('rasterEditor.raw')" />
    </RasterPreviewCard>
    <RasterPreviewCard v-if="materialPreview" :title="t('rasterEditor.layers')">
      <img :src="materialPreview" :alt="t('rasterEditor.layers')" />
    </RasterPreviewCard>
    <RasterPreviewCard
      v-if="profile.startsWith('lot-')"
      :title="t('rasterEditor.lot3d')"
    >
      <RasterLotPreview
        :raw="raw"
        :png="rawPng"
        :width="doc.width"
        :height="doc.height"
        :meters-per-pixel="metersPerPixel"
        :colors="colors"
        :target="target"
        :texture="profile === 'lot-texture'"
      />
    </RasterPreviewCard>
  </section>
</template>
<style scoped>
.preview-panel {
  padding: 16px;
  border-top: 1px solid var(--border);
}
img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  image-rendering: pixelated;
  background: repeating-conic-gradient(#80808030 0% 25%, transparent 0% 50%)
    0/16px 16px;
}
</style>
