<script setup lang="ts">
import { computed } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import type {
  ResourcePreview,
  TextPreview,
  ImagePreview,
  RasterPreview,
  PropertyPreview,
  Rw4Preview,
  AudioPreview,
  VideoPreview,
  FontPreview,
} from "@/api/tauri";
import TextPreviewView from "./TextPreview.vue";
import ImagePreviewView from "./ImagePreview.vue";
import RasterPreviewView from "./RasterPreview.vue";
import PropertyPreviewView from "./PropertyPreview.vue";
import Rw4PreviewView from "./Rw4Preview.vue";
import AudioPreviewView from "./AudioPreview.vue";
import VideoPreviewView from "./VideoPreview.vue";
import FontPreviewView from "./FontPreview.vue";

interface Props {
  preview: ResourcePreview | null;
  typeName?: string;
  loading?: boolean;
  error?: string;
}
const props = withDefaults(defineProps<Props>(), {
  typeName: "",
  loading: false,
  error: "",
});
const textPreview = computed(() =>
  props.preview?.kind === "text" ? (props.preview as TextPreview) : null,
);
const imagePreview = computed(() =>
  props.preview?.kind === "image" ? (props.preview as ImagePreview) : null,
);
const rasterPreview = computed(() =>
  props.preview?.kind === "raster" ? (props.preview as RasterPreview) : null,
);
const propertyPreview = computed(() =>
  props.preview?.kind === "property"
    ? (props.preview as PropertyPreview)
    : null,
);
const rw4Preview = computed(() =>
  props.preview?.kind === "rw4" ? (props.preview as Rw4Preview) : null,
);
const audioPreview = computed(() =>
  props.preview?.kind === "audio" ? (props.preview as AudioPreview) : null,
);
const videoPreview = computed(() =>
  props.preview?.kind === "video" ? (props.preview as VideoPreview) : null,
);
const fontPreview = computed(() =>
  props.preview?.kind === "font" ? (props.preview as FontPreview) : null,
);
const unsupported = computed(
  () =>
    props.preview !== null &&
    !textPreview.value &&
    !imagePreview.value &&
    !rasterPreview.value &&
    !propertyPreview.value &&
    !rw4Preview.value &&
    !audioPreview.value &&
    !videoPreview.value &&
    !fontPreview.value,
);
</script>

<template>
  <section class="resource-preview" aria-live="polite">
    <div v-if="props.loading" class="preview-loading">
      <FSkeleton height="18px" width="42%" rounded />
      <FSkeleton height="12px" width="72%" />
      <FSkeleton height="12px" width="58%" />
      <FSpinner size="sm" :label="$t('common.loading')" />
    </div>
    <p v-else-if="props.error" class="preview-error" role="alert">
      {{ props.error }}
    </p>
    <TextPreviewView v-else-if="textPreview" :preview="textPreview" />
    <ImagePreviewView v-else-if="imagePreview" :preview="imagePreview" />
    <RasterPreviewView v-else-if="rasterPreview" :preview="rasterPreview" />
    <PropertyPreviewView
      v-else-if="propertyPreview"
      :preview="propertyPreview"
    />
    <Rw4PreviewView v-else-if="rw4Preview" :preview="rw4Preview" />
    <AudioPreviewView v-else-if="audioPreview" :preview="audioPreview" />
    <VideoPreviewView v-else-if="videoPreview" :preview="videoPreview" />
    <FontPreviewView v-else-if="fontPreview" :preview="fontPreview" />
    <div v-else-if="unsupported" class="preview-empty">
      <FIcon name="FileQuestion" :size="26" aria-label="" />
      <FTypography :header="4" spacing="none">{{
        $t("package.previewUnavailableType", {
          type: props.typeName || $t("package.other"),
        })
      }}</FTypography>
    </div>
    <div v-else class="preview-empty">
      <FTypography :header="4" spacing="none">{{
        $t("package.noPreview")
      }}</FTypography>
    </div>
  </section>
</template>

<style scoped>
.resource-preview {
  min-height: 300px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
  padding-top: 14px;
}
.preview-loading {
  display: grid;
  align-content: center;
  justify-items: center;
  gap: 16px;
  min-height: 300px;
  padding: 24px;
}
.preview-error {
  color: var(--danger);
  font-size: 12px;
  margin: auto;
  padding: 24px;
}
.preview-empty {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
  min-height: 300px;
  padding: 24px;
  text-align: center;
}
.preview-empty p {
  font-size: 12px;
  line-height: 1.5;
  margin: 0;
  max-width: 340px;
}
</style>
