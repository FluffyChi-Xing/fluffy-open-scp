<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import type * as Three from "three";
import { installHejlToneMapping } from "@/lib/hejlTonemapping";
import type { RasterDocument } from "@/lib/raster-editor/document";
import { createGraffitiProjection } from "./graffitiProjection";

const props = defineProps<{
  doc: RasterDocument;
  version: number;
  colors: string[];
  aspectRatio: number;
}>();
const { t } = useI18n();
const host = useTemplateRef<HTMLDivElement>("host");
const night = shallowRef(false);
const failed = shallowRef(false);
const aspect = computed(() => props.aspectRatio || props.doc.width / props.doc.height);
let renderer: Three.WebGLRenderer | undefined;
let camera: Three.OrthographicCamera | undefined;
let projection: Awaited<ReturnType<typeof createGraffitiProjection>> = null;
let observer: ResizeObserver | undefined;
let rebuild: (() => Promise<void>) | undefined;
let revision = 0;
let disposed = false;
let lastWidth = 0;
let lastHeight = 0;

function draw() {
  const width = host.value?.clientWidth ?? 0;
  const height = host.value?.clientHeight ?? 0;
  if (!renderer || !camera || !projection || !width || !height) return;
  if (width !== lastWidth || height !== lastHeight) {
    const viewport = width / height;
    const halfHeight = 1.12 * Math.max(1, aspect.value / viewport);
    const halfWidth = halfHeight * viewport;
    camera.left = -halfWidth;
    camera.right = halfWidth;
    camera.top = halfHeight;
    camera.bottom = -halfHeight;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height);
    lastWidth = width;
    lastHeight = height;
  }
  renderer.render(projection.scene, camera);
}

onMounted(async () => {
  try {
    const THREE = await import("three");
    if (!host.value || disposed) return;
    renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    installHejlToneMapping(THREE);
    renderer.toneMapping = THREE.CustomToneMapping;
    renderer.toneMappingExposure = 1.12;
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    host.value.appendChild(renderer.domElement);
    camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.01, 10);
    camera.position.z = 3;
    const anisotropy = renderer.capabilities.getMaxAnisotropy();
    rebuild = async () => {
      const job = ++revision;
      const isStale = () => disposed || job !== revision;
      try {
        const next = await createGraffitiProjection(THREE, props.doc, props.colors, aspect.value, anisotropy, isStale);
        if (isStale()) {
          next?.dispose();
          return;
        }
        projection?.dispose();
        projection = next;
        lastWidth = 0;
        projection?.updatePixels(props.doc);
        projection?.setTime(night.value ? 0 : 12);
        draw();
      } catch {
        if (!isStale()) failed.value = true;
      }
    };
    observer = new ResizeObserver(draw);
    observer.observe(host.value);
    await rebuild();
  } catch {
    if (!disposed) failed.value = true;
  }
});

watch(() => [props.doc, aspect.value, ...props.colors], () => void rebuild?.());
watch(() => props.version, () => {
  projection?.updatePixels(props.doc);
  draw();
});
watch(night, () => {
  projection?.setTime(night.value ? 0 : 12);
  draw();
});
onBeforeUnmount(() => {
  disposed = true;
  observer?.disconnect();
  projection?.dispose();
  renderer?.dispose();
  renderer?.forceContextLoss();
  renderer?.domElement.remove();
});
</script>

<template>
  <div class="graffiti-preview">
    <p v-if="failed" role="alert">{{ t("rasterEditor.decalPreviewError") }}</p>
    <template v-else>
      <div ref="host" class="decal-image" role="img" :aria-label="t('rasterEditor.graffiti.preview')" />
      <div class="preview-lighting" role="group" :aria-label="t('rasterEditor.graffiti.lighting')">
        <button class="sign-control" type="button" :aria-pressed="!night" @click="night = false">
          {{ t("rasterEditor.graffiti.day") }}
        </button>
        <button class="sign-control" type="button" :aria-pressed="night" @click="night = true">
          {{ t("rasterEditor.graffiti.night") }}
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.graffiti-preview {
  display: grid;
  block-size: 100%;
  grid-template-rows: minmax(120px, 1fr) auto;
  gap: var(--sign-row-gap, 8px);
}
.decal-image {
  overflow: hidden;
  border-radius: 6px;
  background: var(--surface);
  outline: 1px solid oklch(1 0 0 / 0.1);
  outline-offset: -1px;
}
.decal-image :deep(canvas) { display: block; }
.preview-lighting { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--sign-row-gap, 8px); }
.preview-lighting button {
  min-block-size: var(--sign-control-height, 32px);
  border: 1px solid var(--border);
  border-radius: var(--sign-control-radius, 6px);
  background: var(--surface);
  color: var(--foreground);
  font-size: 12px;
  cursor: pointer;
}
.preview-lighting button[aria-pressed="true"] { background: var(--accent); border-color: var(--primary); }
.preview-lighting button:focus-visible { outline: 2px solid var(--primary); outline-offset: 1px; }
p { margin: 0; font-size: 11px; color: var(--muted-foreground); }
</style>
