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
} from "@/lib/raster-editor/encoders";
import {
  channelPixels,
  type RasterProfile,
} from "@/lib/raster-editor/schema";
import { composeGroundPixels } from "@/pages/packages/components/property-editor/groundCompose";
const props = defineProps<{
  target: RasterSource | null;
  metersPerPixel: number;
  doc: RasterDocument;
  version: number;
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
      PROFILE_BEHAVIOR[props.profile].dataChannels
        ? dataChannelPixels(props.doc.pixels)
        : props.doc.pixels,
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
const currentLotRaw = computed(() => {
  void previewVersion.value;
  return rgbaToBase64(props.doc.pixels);
});
const currentLotPng = computed(() =>
  rgbaBase64ToPngDataUrl(currentLotRaw.value, props.doc.width, props.doc.height),
);
const currentLotAlbedoPng = computed(() => {
  void previewVersion.value;
  const output = composeGroundPixels({
    mask: {
      data: props.doc.pixels,
      width: props.doc.width,
      height: props.doc.height,
    },
    rawMask: {
      data: props.doc.pixels,
      width: props.doc.width,
      height: props.doc.height,
    },
    baseTile: null,
    lotColors: props.colors,
    lotBorderColors: null,
    lotBorderPatternIndices: null,
    lotBorderWidths: null,
    normalAtlas: null,
    tilesX: 1,
    tilesY: 1,
  });
  return rgbaBase64ToPngDataUrl(
    rgbaToBase64(output.albedo),
    output.width,
    output.height,
  );
});
</script>
<template>
  <section class="preview-panel">
    <RasterPreviewCard
      :title="
        t(
          PROFILE_BEHAVIOR[profile].dataChannels
            ? 'rasterEditor.dataRgb'
            : 'rasterEditor.raw',
        )
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
    <RasterPreviewCard
      v-if="profile.startsWith('lot-')"
      :title="t('rasterEditor.layers')"
    >
      <RasterLotPreview
        :raw="currentLotRaw"
        :png="currentLotPng"
        :width="doc.width"
        :height="doc.height"
        :meters-per-pixel="metersPerPixel"
        :colors="colors"
        :target="target"
        :texture="profile === 'lot-texture'"
        :albedo-png="currentLotAlbedoPng"
        render-mode="default"
      />
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
        render-mode="refined"
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
