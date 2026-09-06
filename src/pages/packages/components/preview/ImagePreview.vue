<script setup lang="ts">
import { ref, toRef, watch } from "vue";
import type { ImagePreview as ImagePreviewData } from "@/api/tauri";
import {
  defaultImageFormat,
  useResourceExport,
  type ResourceExportFormat,
} from "@/composables/useResourceExport";

const props = defineProps<{ preview: ImagePreviewData }>();
const scale = ref(1);
const rotation = ref(0);
const imageFormat = ref<ResourceExportFormat>(defaultImageFormat(props.preview.mime));
const { exporting, exportResource } = useResourceExport(toRef(props, "preview"));
watch(
  () => props.preview.mime,
  (mime) => {
    imageFormat.value = defaultImageFormat(mime);
  },
);
function zoom(delta: number) {
  scale.value = Math.min(
    4,
    Math.max(0.25, Number((scale.value + delta).toFixed(2))),
  );
}
function reset() {
  scale.value = 1;
  rotation.value = 0;
}
function onWheel(event: WheelEvent) {
  event.preventDefault();
  zoom(event.deltaY < 0 ? 0.1 : -0.1);
}
function onKeydown(event: KeyboardEvent) {
  if (event.key === "+" || event.key === "=") zoom(0.1);
  if (event.key === "-") zoom(-0.1);
  if (event.key.toLowerCase() === "r")
    rotation.value = (rotation.value + 90) % 360;
}
</script>

<template>
  <div class="image-preview">
    <div
      class="image-toolbar"
      role="toolbar"
      :aria-label="$t('package.imageToolbar')"
    >
      <button
        type="button"
        :aria-label="$t('package.zoomOut')"
        :disabled="scale <= 0.25"
        @click="zoom(-0.1)"
      >
        −
      </button>
      <span>{{ Math.round(scale * 100) }}%</span>
      <button
        type="button"
        :aria-label="$t('package.zoomIn')"
        :disabled="scale >= 4"
        @click="zoom(0.1)"
      >
        +
      </button>
      <button
        type="button"
        :aria-label="$t('package.rotateImage')"
        @click="rotation = (rotation + 90) % 360"
      >
        ↻
      </button>
      <button
        type="button"
        :aria-label="$t('package.exportResource')"
        :disabled="exporting"
        @click="exportResource(imageFormat)"
      >
        {{ exporting ? $t("package.exporting") : $t("package.export") }}
      </button>
      <select
        v-model="imageFormat"
        :aria-label="$t('package.exportFormat')"
        :disabled="exporting"
      >
        <option value="png">PNG</option>
        <option value="jpg">JPG</option>
        <option value="gif">GIF</option>
      </select>
      <button
        type="button"
        :aria-label="$t('package.resetImage')"
        @click="reset"
      >
        {{ $t("package.reset") }}
      </button>
    </div>
    <div
      class="image-viewport"
      tabindex="0"
      @wheel="onWheel"
      @keydown="onKeydown"
    >
      <img
        :src="props.preview.src"
        :alt="$t('package.imagePreview')"
        :style="{ transform: `scale(${scale}) rotate(${rotation}deg)` }"
        draggable="false"
      />
    </div>
    <div class="image-meta">
      {{ preview.width ?? "—" }} × {{ preview.height ?? "—" }} ·
      {{ preview.mime }}
    </div>
  </div>
</template>

<style scoped>
.image-preview {
  display: grid;
  gap: 10px;
  min-width: 0;
}
.image-toolbar {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  gap: 5px;
  padding: 5px;
}
.image-toolbar button {
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  min-height: 28px;
  min-width: 28px;
  padding: 0 7px;
}
.image-toolbar button:hover,
.image-toolbar button:focus-visible {
  background: var(--accent);
  border-color: var(--border);
}
.image-toolbar button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.image-toolbar span {
  color: var(--muted-foreground);
  font-size: 11px;
  min-width: 42px;
  text-align: center;
}
.image-toolbar select {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 11px;
  min-height: 28px;
  padding: 0 6px;
}
.image-toolbar select:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
.image-toolbar button:active,
.image-toolbar select:active {
  transform: scale(0.96);
}

.image-viewport {
  align-items: center;
  background: repeating-conic-gradient(
      var(--surface-hover) 0 25%,
      var(--surface-elevated) 0 50%
    )
    50% / 20px 20px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  justify-content: center;
  min-height: 300px;
  overflow: hidden;
  padding: 30px;
}
.image-viewport img {
  box-shadow: var(--shadow-md);
  max-height: 330px;
  max-width: 100%;
  object-fit: contain;
  transition: transform 120ms ease;
  user-select: none;
}
.image-meta {
  color: var(--muted-foreground);
  font-size: 11px;
}
</style>
