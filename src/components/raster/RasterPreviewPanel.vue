<script setup lang="ts">
import { useI18n } from "vue-i18n";
import RasterPreviewCard from "./RasterPreviewCard.vue";
import RasterLotPreview from "./RasterLotPreview.vue";
import RasterDecalPreview from "./RasterDecalPreview.vue";
import type { RasterSource } from "@/lib/raster-editor/schema";
import { computed, onBeforeUnmount, shallowRef, watch } from "vue";
import {
  PROFILE_BEHAVIOR,
  dataChannelPixels,
} from "@/lib/raster-editor/profileBehavior";
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
  decalColors: string[];
  decalAspectRatio: number;
}>();
const { t } = useI18n();
const previewVersion = shallowRef(props.version);
let previewTimer: ReturnType<typeof setTimeout> | null = null;
watch(() => props.version, (value) => {
  if (previewTimer) clearTimeout(previewTimer);
  previewTimer = setTimeout(() => {
    previewVersion.value = value;
    previewTimer = null;
  }, 180);
});
onBeforeUnmount(() => {
  if (previewTimer) clearTimeout(previewTimer);
});
type LotSnapshot = {
  raw: string;
  png: string;
  width: number;
  height: number;
  metersPerPixel: number;
  colors: [number, number, number, number][];
  target: RasterSource | null;
  texture: boolean;
};
const lotSnapshot = shallowRef<LotSnapshot | null>(null);
function refreshLotPreview() {
  const raw = rgbaToBase64(props.doc.pixels);
  lotSnapshot.value = {
    raw,
    png: rgbaBase64ToPngDataUrl(raw, props.doc.width, props.doc.height),
    width: props.doc.width,
    height: props.doc.height,
    metersPerPixel: props.metersPerPixel,
    colors: props.colors.map((color) => [...color] as [number, number, number, number]),
    target: props.target ? { ...props.target, tgi: props.target.tgi ? { ...props.target.tgi } : undefined } : null,
    texture: props.profile === "lot-texture",
  };
}
watch(() => [props.doc, props.profile, props.target], () => { lotSnapshot.value = null; });
const preview = computed(() => {
  void previewVersion.value;
  return rgbaBase64ToPngDataUrl(
    rgbaToBase64(
      props.channel === "rgba" && PROFILE_BEHAVIOR[props.profile].dataChannels
        ? dataChannelPixels(props.doc.pixels)
        : channelPixels(props.doc.pixels, props.channel),
    ),
    props.doc.width,
    props.doc.height,
  );
});
const alphaPreview = computed(() => {
  void previewVersion.value;
  return rgbaBase64ToPngDataUrl(
    rgbaToBase64(channelPixels(props.doc.pixels, "a")),
    props.doc.width,
    props.doc.height,
  );
});
const materialPreview = computed(() => {
  void previewVersion.value;
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
          ? t(
              PROFILE_BEHAVIOR[profile].dataChannels
                ? 'rasterEditor.dataRgb'
                : 'rasterEditor.raw',
            )
          : t('rasterEditor.channel', { channel: channel.toUpperCase() })
      "
    >
      <img :src="preview" :alt="t('rasterEditor.raw')" />
    </RasterPreviewCard>
    <RasterPreviewCard
      v-if="PROFILE_BEHAVIOR[profile].dataChannels"
      :title="t('rasterEditor.dataAlpha')"
    >
      <img :src="alphaPreview" :alt="t('rasterEditor.dataAlpha')" />
    </RasterPreviewCard>
    <RasterPreviewCard
      v-if="profile.startsWith('decal-')"
      :title="t(`rasterEditor.previewTitle.${profile}`)"
    >
      <RasterDecalPreview
        :doc="doc"
        :version="version"
        :profile="profile"
        :colors="decalColors"
        :aspect-ratio="decalAspectRatio"
      />
    </RasterPreviewCard>
    <RasterPreviewCard v-if="materialPreview" :title="t('rasterEditor.layers')">
      <img :src="materialPreview" :alt="t('rasterEditor.layers')" />
    </RasterPreviewCard>
    <RasterPreviewCard
      v-if="profile.startsWith('lot-')"
      :title="t('rasterEditor.lot3d')"
      refreshable
      @refresh="refreshLotPreview"
    >
      <RasterLotPreview
        v-if="lotSnapshot"
        :raw="lotSnapshot.raw"
        :png="lotSnapshot.png"
        :width="lotSnapshot.width"
        :height="lotSnapshot.height"
        :meters-per-pixel="lotSnapshot.metersPerPixel"
        :colors="lotSnapshot.colors"
        :target="lotSnapshot.target"
        :texture="lotSnapshot.texture"
      />
      <p v-else class="refresh-hint" role="status">{{ t('rasterEditor.refreshPreviewHint') }}</p>
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
.refresh-hint {
  display: grid;
  place-items: center;
  height: 100%;
  margin: 0;
  padding: 16px;
  color: var(--muted-foreground);
  font-size: 12px;
  text-align: center;
}
</style>
