<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type * as Three from "three";
import { createEngineDecalMaterial } from "@/lib/decalEngineMaterials";
import type { RasterDocument } from "@/lib/raster-editor/document";
import type { RasterProfile } from "@/lib/raster-editor/schema";
import { signAnimationParameter, type SignSettings } from "@/lib/raster-editor/signPaint";
import {
  decalPreviewFamily,
  decalPreviewPixels,
} from "@/lib/raster-editor/decalPreview";

const props = defineProps<{
  doc: RasterDocument;
  version: number;
  profile: RasterProfile;
  colors: string[];
  aspectRatio: number;
  sign?: SignSettings;
}>();
const { t } = useI18n();
const host = ref<HTMLElement>();
const failed = ref(false);
const playing = ref(false);
const time = { value: 0 };
const animEnabled = { value: 0 };
let frame = 0;
let disposed = false;
let observer: ResizeObserver | undefined;
let renderer: Three.WebGLRenderer | undefined;
let draw: (() => void) | undefined;
let release: (() => void) | undefined;
let renderFrame: (() => void) | undefined;

function stopPlayback() {
  cancelAnimationFrame(frame);
  frame = 0;
  animEnabled.value = 0;
  playing.value = false;
  renderFrame?.();
}
function togglePlayback() {
  if (playing.value) {
    stopPlayback();
    return;
  }
  if (!renderFrame) return;
  const start = performance.now();
  playing.value = true;
  animEnabled.value = 1;
  time.value = 0;
  const tick = (now: number) => {
    time.value = (now - start) / 1000;
    renderFrame?.();
    frame = requestAnimationFrame(tick);
  };
  frame = requestAnimationFrame(tick);
}

onMounted(async () => {
  try {
    const THREE = await import("three");
    if (disposed || !host.value) return;
    renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    renderer.setClearColor(0, 0);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    host.value.appendChild(renderer.domElement);
    const scene = new THREE.Scene();
    const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 10);
    camera.position.z = 2;
    renderFrame = () => renderer?.render(scene, camera);
    const geometry = new THREE.PlaneGeometry(2, 2);
    let texture: Three.DataTexture | undefined;
    let material: Three.ShaderMaterial | undefined;
    let mesh: Three.Mesh | undefined;
    release = () => {
      texture?.dispose();
      material?.dispose();
      geometry.dispose();
    };
    draw = () => {
      if (!host.value || disposed || !renderer) return;
      const { width, height } = host.value.getBoundingClientRect();
      if (!width || !height) return;
      texture?.dispose();
      material?.dispose();
      if (mesh) scene.remove(mesh);
      // Upload raw channels without browser alpha premultiplication or color conversion.
      texture = new THREE.DataTexture(
        decalPreviewPixels(props.doc),
        props.doc.width,
        props.doc.height,
      );
      texture.minFilter = texture.magFilter = THREE.LinearFilter;
      texture.needsUpdate = true;
      const sign = props.sign;
      const marquee = sign?.effect === "marquee";
      material = createEngineDecalMaterial(
        THREE,
        sign ? (marquee ? "sdf" : "sign") : decalPreviewFamily(props.profile),
        {
          map: texture,
          layerColors: props.colors.map((hex, index) => {
            const color = new THREE.Color(hex);
            // Engine material consumes doubled DTO animation values.
            const animation = marquee ? 2 * signAnimationParameter(sign.axis, index) : 0;
            return [color.r, color.g, color.b, animation];
          }),
          ...(sign ? { materialInfo: [0.4, marquee ? sign.speed : 0, 0.1] } : {}),
          graffiti: props.profile === "decal-graffiti",
          decalData: null,
          worldDirection: new THREE.Vector3(0, 0, 1),
          env: {
            sunDir: { value: new THREE.Vector3(0, 0, 1) },
            sunColor: { value: new THREE.Color(1, 1, 1) },
            dayLight: { value: 1 },
            nightBoost: { value: 1 },
            decalGlow: { value: 2.5 },
            animEnabled,
            time,
          },
          side: THREE.DoubleSide,
        },
      );
      mesh = new THREE.Mesh(geometry, material);
      const ratio = Math.max(
        0.01,
        props.aspectRatio || props.doc.width / props.doc.height,
      );
      const viewportRatio = width / height;
      mesh.scale.set(
        Math.min(1, ratio / viewportRatio),
        Math.min(1, viewportRatio / ratio),
        1,
      );
      scene.add(mesh);
      renderer.setSize(width, height);
      renderFrame?.();
    };
    observer = new ResizeObserver(() => draw?.());
    observer.observe(host.value);
    draw();
  } catch {
    if (!disposed) failed.value = true;
  }
});
watch(
  () => [
    props.doc,
    props.version,
    props.profile,
    ...props.colors,
    props.aspectRatio,
    props.sign,
  ],
  () => draw?.(),
);
watch(() => props.sign?.effect, (effect) => {
  if (effect !== "marquee") stopPlayback();
});
onBeforeUnmount(() => {
  disposed = true;
  renderFrame = undefined;
  stopPlayback();
  observer?.disconnect();
  release?.();
  renderer?.dispose();
  renderer?.forceContextLoss();
  renderer?.domElement.remove();
});
</script>
<template>
  <div class="decal-preview" :class="{ 'sign-preview': sign }">
    <p v-if="failed" role="alert">{{ t("rasterEditor.decalPreviewError") }}</p>
    <div
      v-else
      ref="host"
      class="decal-image"
      role="img"
      :aria-label="t('rasterEditor.decalPreview')"
    />
    <button v-if="sign?.effect === 'marquee' && !failed" type="button" @click="togglePlayback">
      {{ t(playing ? "rasterEditor.sign.pause" : "rasterEditor.sign.play") }}
    </button>
    <p v-if="!sign">{{ t(`rasterEditor.profileGuide.${profile}`) }}</p>
  </div>
</template>
<style scoped>
.decal-preview {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.decal-image {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background: repeating-conic-gradient(#80808030 0% 25%, transparent 0% 50%)
    0/16px 16px;
}
.decal-image :deep(canvas) {
  display: block;
}
.sign-preview { gap: 8px; }
.sign-preview .decal-image {
  flex: auto;
  height: 120px;
  border-radius: 6px;
  background: #171f2c;
  outline: 1px solid oklch(1 0 0 / 0.1);
  outline-offset: -1px;
}
.sign-preview button {
  align-self: start;
  padding: 5px 9px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.sign-preview button:focus-visible { outline: 2px solid var(--primary); }
p {
  margin: 6px 0 0;
  font-size: 11px;
  color: var(--muted-foreground);
}
</style>
