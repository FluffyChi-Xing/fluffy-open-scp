<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import { createDataSource } from "@/api/data-source";
import {
  RASTER_CHANNELS,
  type ImagePreview as ImagePreviewData,
  type RasterChannel,
  type RasterPreview,
} from "@/api/tauri";
import ImagePreviewView from "./ImagePreview.vue";

const props = defineProps<{ preview: RasterPreview }>();
const { t } = useI18n();
const source = createDataSource();

const channel = ref<RasterChannel>(props.preview.channel);
/** 视图 → data URL；切换过的视图缓存复用。 */
const rendered = ref<Partial<Record<RasterChannel, string>>>({});
/** 当前显示中的图，切换期间保持旧图避免闪烁。 */
const displayed = ref<string | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);

const CHANNEL_LABELS: Record<RasterChannel, string> = {
  quantized: "raster.quantized",
  composite: "raster.composite",
  r: "raster.red",
  g: "raster.green",
  b: "raster.blue",
  a: "raster.alpha",
};

const resourceKey = computed(
  () =>
    `${props.preview.tgi.typeId}:${props.preview.tgi.group}:${props.preview.tgi.instance}`,
);

const hintText = computed(() => {
  if (channel.value === "quantized") return t("raster.quantizedHint");
  if (channel.value === "composite") return t("raster.compositeHint");
  return t("raster.singleChannelHint", {
    channel: t(CHANNEL_LABELS[channel.value]),
  });
});

const activePreview = computed<ImagePreviewData | null>(() => {
  const src = displayed.value;
  if (!src) return null;
  return {
    kind: "image",
    packageId: props.preview.packageId,
    tgi: props.preview.tgi,
    offset: 0,
    totalLength: props.preview.totalLength,
    bytes: [],
    src,
    mime: "image/png",
    width: props.preview.width,
    height: props.preview.height,
    // 原图常小到 32×32，放大观察必须最近邻，否则会被插值糊掉。
    pixelated: true,
  };
});

function seed() {
  channel.value = props.preview.channel;
  rendered.value = props.preview.pngBase64
    ? {
        [props.preview.channel]:
          `data:image/png;base64,${props.preview.pngBase64}`,
      }
    : {};
  displayed.value = rendered.value[props.preview.channel] ?? null;
  error.value = null;
  loading.value = false;
}

async function select(next: RasterChannel) {
  channel.value = next;
  const cached = rendered.value[next];
  if (cached) {
    displayed.value = cached;
    error.value = null;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const data = await source.readRasterPreview(
      props.preview.packageId,
      props.preview.tgi,
      next,
    );
    if (data.pngBase64) {
      const src = `data:image/png;base64,${data.pngBase64}`;
      rendered.value = { ...rendered.value, [next]: src };
      displayed.value = src;
    } else {
      error.value = t("raster.unsupportedFormat", {
        format: data.pixelFormat,
      });
    }
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    loading.value = false;
  }
}

watch(resourceKey, seed, { immediate: true });
</script>

<template>
  <div class="raster-preview">
    <div
      class="raster-channels"
      role="tablist"
      :aria-label="$t('raster.channel')"
    >
      <button
        v-for="item in RASTER_CHANNELS"
        :key="item"
        type="button"
        role="tab"
        :aria-selected="channel === item"
        :class="{ active: channel === item }"
        @click="select(item)"
      >
        {{ $t(CHANNEL_LABELS[item]) }}
      </button>
    </div>

    <p class="raster-hint">
      <FIcon
        v-if="loading"
        name="LoaderCircle"
        :size="12"
        aria-label=""
        class="raster-spin"
      />
      {{ hintText }}
    </p>

    <p class="raster-meta">
      <span>{{ preview.width }} × {{ preview.height }}</span>
      <span>pixFmt {{ preview.pixelFormat }}</span>
      <span>{{ $t("raster.mipCount", { count: preview.mipCount }) }}</span>
    </p>

    <ImagePreviewView v-if="activePreview" :preview="activePreview" />
    <p v-else-if="error" class="raster-error" role="alert">{{ error }}</p>
    <p v-else class="raster-error">{{ $t("raster.loadFailed") }}</p>
  </div>
</template>

<style scoped>
.raster-preview {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}
.raster-channels {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.raster-channels button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  min-height: 26px;
  padding: 0 10px;
}
.raster-channels button:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.raster-channels button.active {
  background: var(--surface-hover);
  border-color: color-mix(in srgb, var(--border) 50%, var(--foreground));
  color: var(--foreground);
  font-weight: 600;
}
.raster-channels button:focus-visible {
  outline: 2px solid var(--accent, #2563eb);
  outline-offset: 1px;
}
.raster-hint {
  align-items: center;
  color: var(--subtle-foreground);
  display: flex;
  font-size: 11px;
  gap: 5px;
  margin: 0;
}
.raster-meta {
  color: var(--subtle-foreground);
  display: flex;
  flex-wrap: wrap;
  font-size: 10px;
  gap: 4px 12px;
  margin: 0;
}
.raster-spin {
  animation: raster-spin 900ms linear infinite;
}
@keyframes raster-spin {
  to {
    transform: rotate(360deg);
  }
}
.raster-error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  padding: 20px 12px;
  text-align: center;
}
@media (prefers-reduced-motion: reduce) {
  .raster-spin {
    animation: none;
  }
}
</style>
