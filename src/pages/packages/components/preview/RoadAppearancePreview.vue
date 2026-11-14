<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { command, type Tgi } from "@/api/tauri";
import {
  createRoadAppearanceScene,
  disposeRoadAppearance,
  type RoadAppearance,
} from "./roadAppearanceScene";

const props = defineProps<{ packageId: number; tgi: Tgi }>();
const { t } = useI18n();
const host = ref<HTMLDivElement | null>(null);
const status = ref<"loading" | "empty" | "error" | "ready">("loading");
const unresolved = ref(0);

watch(
  () => [host.value, props.packageId, props.tgi] as const,
  async ([element, packageId, tgi], _, onCleanup) => {
    if (!element) return;
    status.value = "loading";
    unresolved.value = 0;
    let cancelled = false;
    let release: (() => void) | undefined;
    onCleanup(() => {
      cancelled = true;
      release?.();
    });
    try {
      const assets = await command<RoadAppearance>("read_road_appearance", {
        packageId,
        tgi,
      });
      if (cancelled) return;
      const road = await createRoadAppearanceScene(assets, tgi.instance);
      if (cancelled) {
        disposeRoadAppearance(road);
        return;
      }
      release = () => disposeRoadAppearance(road);
      const bounds = new THREE.Box3().setFromObject(road);
      if (bounds.isEmpty()) {
        status.value = "empty";
        return;
      }

      const renderer = new THREE.WebGLRenderer({
        antialias: true,
        alpha: true,
      });
      release = () => {
        renderer.dispose();
        disposeRoadAppearance(road);
      };
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      const scene = new THREE.Scene();
      const daylight = new THREE.DirectionalLight(0xffffff, 2.1);
      daylight.position.set(-40, 80, 50);
      scene.add(
        road,
        daylight,
        new THREE.HemisphereLight(0xeaf1ff, 0x526377, 1.5),
      );
      const camera = new THREE.PerspectiveCamera(42, 1, 0.1, 2000);
      const center = bounds.getCenter(new THREE.Vector3());
      const span = bounds.getSize(new THREE.Vector3()).length();
      const controls = new OrbitControls(camera, renderer.domElement);
      controls.target.copy(center);
      controls.maxDistance = span * 4;
      const draw = () => renderer.render(scene, camera);
      const observer = new ResizeObserver(() => {
        if (!element.clientWidth || !element.clientHeight) return;
        renderer.setSize(element.clientWidth, element.clientHeight);
        camera.aspect = element.clientWidth / element.clientHeight;
        camera.updateProjectionMatrix();
        draw();
      });
      release = () => {
        observer.disconnect();
        controls.dispose();
        renderer.dispose();
        renderer.domElement.remove();
        disposeRoadAppearance(road);
      };
      element.appendChild(renderer.domElement);
      observer.observe(element);
      camera.position
        .copy(center)
        .addScaledVector(new THREE.Vector3(0.6, 0.9, 1), span);
      controls.update();
      controls.addEventListener("change", draw);
      unresolved.value = new Set(assets.unsupportedComponents).size;
      status.value = "ready";
    } catch {
      release?.();
      release = undefined;
      if (!cancelled) status.value = "error";
    }
  },
  { immediate: true, flush: "post" },
);
</script>

<template>
  <section
    class="road-appearance"
    :aria-label="t('roadAsset.appearance.view3d')"
  >
    <div ref="host" class="viewport" />
    <p v-if="status !== 'ready'" class="status" role="status">
      {{ t(`roadAsset.appearance.${status}`) }}
    </p>
    <p v-else-if="unresolved" class="status">
      {{ t("roadAsset.appearance.partial", { count: unresolved }) }}
    </p>
  </section>
</template>

<style scoped>
.road-appearance {
  position: relative;
  min-block-size: 260px;
  flex: 1;
  border: 1px solid var(--border);
  border-radius: 6px;
  overflow: hidden;
  background: var(--surface-elevated);
}
.viewport {
  position: absolute;
  inset: 0;
}
.viewport :deep(canvas) {
  display: block;
}
.status {
  position: absolute;
  inset-inline: 10px;
  inset-block-start: 8px;
  margin: 0;
  color: var(--muted-foreground);
  font-size: 12px;
  pointer-events: none;
}
</style>
