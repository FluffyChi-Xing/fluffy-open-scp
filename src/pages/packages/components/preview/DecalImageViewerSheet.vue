<script setup lang="ts">
import { computed, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSheet from "@/components/ui/FSheet.vue";
import type {
  DecalEntryMeta,
  DecalImageData,
  ImagePreview as ImagePreviewData,
} from "@/api/tauri";
import ImagePreviewView from "./ImagePreview.vue";

const props = defineProps<{
  /** 当前检索结果，前后切换在此范围内进行。 */
  entries: DecalEntryMeta[];
  /** 已解码的条目（下标 → 图片数据），由相册负责填充。 */
  images: Map<number, DecalImageData>;
  /** 打开时定位到的条目下标。 */
  startIndex: number;
}>();

const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ request: [index: number] }>();

const current = ref(props.startIndex);

watch(open, (value) => {
  if (!value) return;
  current.value = props.startIndex;
  emit("request", props.startIndex);
});

const position = computed(() =>
  props.entries.findIndex((entry) => entry.index === current.value),
);
const entry = computed(() => props.entries[position.value] ?? null);
const image = computed(() => props.images.get(current.value) ?? null);

/** 解码错误，或该条目本身不可解码的原因（不会一直停在"加载中"）。 */
const stageError = computed(
  () => image.value?.error ?? entry.value?.rasterStatus.reason ?? null,
);

/** 复用通用图片预览（缩放/旋转/重置/滚轮），但不提供资源导出。 */
const preview = computed<ImagePreviewData | null>(() => {
  const payload = image.value;
  if (!payload?.pngBase64) return null;
  return {
    kind: "image",
    offset: 0,
    totalLength: 0,
    bytes: [],
    src: `data:image/png;base64,${payload.pngBase64}`,
    mime: "image/png",
    width: payload.width ?? undefined,
    height: payload.height ?? undefined,
    // 贴花原图很小（32×32 起），放大观察时保持锐利。
    pixelated: true,
  };
});

function hex(value: number | null | undefined) {
  return value === null || value === undefined
    ? "—"
    : `0x${value.toString(16).padStart(8, "0").toUpperCase()}`;
}

function step(delta: number) {
  const next = position.value + delta;
  if (next < 0 || next >= props.entries.length) return;
  current.value = props.entries[next].index;
  emit("request", current.value);
}
</script>

<template>
  <FSheet
    v-model:open="open"
    :label="$t('decal.viewerTitle')"
    width="min(860px, 94vw)"
  >
    <div class="decal-viewer">
      <header class="decal-viewer-head">
        <div class="decal-viewer-title">
          <span>{{ $t("decal.viewerTitle") }}</span>
          <code v-if="entry">{{ hex(entry.id?.instance) }}</code>
        </div>
        <div class="decal-viewer-nav">
          <button
            type="button"
            :aria-label="$t('decal.previous')"
            :disabled="position <= 0"
            @click="step(-1)"
          >
            <FIcon name="ChevronLeft" :size="14" aria-label="" />
          </button>
          <span class="decal-viewer-pos">
            {{ position + 1 }} / {{ entries.length }}
          </span>
          <button
            type="button"
            :aria-label="$t('decal.next')"
            :disabled="position >= entries.length - 1"
            @click="step(1)"
          >
            <FIcon name="ChevronRight" :size="14" aria-label="" />
          </button>
          <button
            type="button"
            :aria-label="$t('decal.close')"
            @click="open = false"
          >
            <FIcon name="X" :size="14" aria-label="" />
          </button>
        </div>
      </header>

      <dl v-if="entry" class="decal-viewer-meta">
        <div>
          <dt>{{ $t("decal.rasterId") }}</dt>
          <dd>
            <code>{{ hex(entry.raster?.instance) }}</code>
          </dd>
        </div>
        <div>
          <dt>{{ $t("decal.aspectRatio") }}</dt>
          <dd>{{ entry.aspectRatio ?? "—" }}</dd>
        </div>
        <div>
          <dt>{{ $t("decal.colorsLabel") }}</dt>
          <dd class="decal-swatches">
            <span
              v-for="(color, slot) in entry.colorsRgba8 ?? []"
              :key="slot"
              class="decal-swatch"
              :style="{
                background: `rgb(${color[0]} ${color[1]} ${color[2]})`,
              }"
              :title="`rgb(${color[0]}, ${color[1]}, ${color[2]})`"
            />
          </dd>
        </div>
      </dl>

      <div class="decal-viewer-stage">
        <ImagePreviewView
          v-if="preview"
          :preview="preview"
          :show-export="false"
        />
        <p v-else-if="stageError" class="decal-viewer-hint" role="alert">
          <FIcon name="TriangleAlert" :size="13" aria-label="" />
          {{ stageError }}
        </p>
        <p v-else class="decal-viewer-hint">
          <FIcon name="LoaderCircle" :size="13" aria-label="" />
          {{ $t("decal.statusLoading") }}
        </p>
      </div>
    </div>
  </FSheet>
</template>

<style scoped>
.decal-viewer {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px 16px 20px;
}
.decal-viewer-head {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
}
.decal-viewer-title {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.decal-viewer-title span {
  color: var(--foreground);
  font-size: 13px;
  font-weight: 600;
}
.decal-viewer-title code {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.decal-viewer-nav {
  align-items: center;
  display: flex;
  gap: 4px;
}
.decal-viewer-nav button {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  height: 26px;
  justify-content: center;
  width: 26px;
}
.decal-viewer-nav button:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--foreground);
}
.decal-viewer-nav button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.decal-viewer-pos {
  color: var(--subtle-foreground);
  font-size: 11px;
  min-width: 52px;
  text-align: center;
}
.decal-viewer-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 18px;
  margin: 0;
}
.decal-viewer-meta div {
  display: flex;
  gap: 6px;
}
.decal-viewer-meta dt {
  color: var(--subtle-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.decal-viewer-meta dd {
  color: var(--foreground);
  font-size: 11px;
  margin: 0;
}
.decal-swatches {
  display: inline-flex;
  gap: 3px;
}
.decal-swatch {
  border: 1px solid var(--border);
  border-radius: 2px;
  height: 12px;
  width: 12px;
}
.decal-viewer-stage {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  justify-content: center;
  min-height: 320px;
  overflow: auto;
}
.decal-viewer-hint {
  align-items: center;
  color: var(--subtle-foreground);
  display: inline-flex;
  font-size: 12px;
  gap: 6px;
  padding: 40px 12px;
}
</style>
