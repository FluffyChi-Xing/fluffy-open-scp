<script setup lang="ts">
import { onBeforeUnmount, onMounted, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import {
  crossSection,
  recipes,
  type RecipeId,
  type Streetlights,
} from "./roadSchema";

const props = defineProps<{ recipe: RecipeId; streetlights: Streetlights }>();
const { t } = useI18n();
const host = shallowRef<HTMLDivElement | null>(null);
const unavailable = shallowRef(false);
let update: (() => void) | undefined;
let dispose: (() => void) | undefined;

onMounted(() => {
  if (!host.value) return;
  const element = host.value;
  let renderer: THREE.WebGLRenderer;
  try {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  } catch {
    unavailable.value = true;
    return;
  }
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  element.appendChild(renderer.domElement);
  const scene = new THREE.Scene();
  scene.add(new THREE.GridHelper(220, 55, 0x4d6081, 0x2c3c57));
  const camera = new THREE.PerspectiveCamera(42, 1, 0.1, 450);
  camera.position.set(44, 42, 63);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.maxPolarAngle = Math.PI / 2 - 0.03;
  controls.maxDistance = 170;
  controls.update();
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const grid = scene.children[0] as THREE.GridHelper;
  const materials = {
    traffic: new THREE.MeshBasicMaterial(),
    walk: new THREE.MeshBasicMaterial({ color: 0x99a4af }),
    median: new THREE.MeshBasicMaterial({ color: 0x54755a }),
    rail: new THREE.MeshBasicMaterial({ color: 0x596673 }),
    margin: new THREE.MeshBasicMaterial({ color: 0x555b62 }),
    marking: new THREE.MeshBasicMaterial({ color: 0xe2d9ba }),
    lamp: new THREE.MeshBasicMaterial({ color: 0xa8b8c6 }),
    track: new THREE.MeshBasicMaterial({ color: 0xa2b5c7 }),
  };
  const road = new THREE.Group();
  scene.add(road);
  const draw = () => renderer.render(scene, camera);
  const box = (
    material: THREE.Material,
    x: number,
    y: number,
    z: number,
    width: number,
    height: number,
    length: number,
  ) => {
    const mesh = new THREE.Mesh(geometry, material);
    mesh.scale.set(width, height, length);
    mesh.position.set(x, y, z);
    road.add(mesh);
  };
  update = () => {
    road.clear();
    materials.traffic.color.setHex(
      recipes[props.recipe].surface === "gravel" ? 0x736556 : 0x394352,
    );
    const parts = crossSection(props.recipe);
    let x = -parts.reduce((width, part) => width + part.width, 0) / 2;
    let previousDirection: string | undefined;
    for (const part of parts) {
      box(
        materials[part.kind],
        x + part.width / 2,
        part.kind === "walk" ? 0.23 : 0.12,
        0,
        part.width,
        0.24,
        90,
      );
      if (part.kind === "rail") {
        for (const offset of [-0.8, 0.8])
          box(
            materials.track,
            x + part.width / 2 + offset,
            0.27,
            0,
            0.11,
            0.06,
            90,
          );
      }
      if (
        part.direction &&
        previousDirection &&
        recipes[props.recipe].surface !== "gravel"
      ) {
        if (part.direction !== previousDirection) {
          box(materials.marking, x, 0.25, 0, 0.17, 0.01, 90);
        } else {
          for (let z = -42; z <= 42; z += 6)
            box(materials.marking, x, 0.25, z, 0.12, 0.01, 2.6);
        }
      }
      previousDirection = part.direction;
      x += part.width;
    }
    if (props.recipe === "highStreet") {
      const sides = [
        {
          x: -11.5,
          visible:
            props.streetlights === "original" || props.streetlights === "left",
        },
        {
          x: 11.5,
          visible:
            props.streetlights === "original" || props.streetlights === "right",
        },
      ];
      for (const side of sides.filter((side) => side.visible)) {
        for (const z of [-33, 0, 33]) {
          box(materials.lamp, side.x, 3.2, z, 0.14, 6, 0.14);
          box(materials.lamp, side.x * 0.94, 6.2, z, 1.5, 0.13, 0.3);
        }
      }
    }
    draw();
  };
  const observer = new ResizeObserver(() => {
    if (!element.clientWidth || !element.clientHeight) return;
    renderer.setSize(element.clientWidth, element.clientHeight);
    camera.aspect = element.clientWidth / element.clientHeight;
    camera.updateProjectionMatrix();
    draw();
  });
  observer.observe(element);
  controls.addEventListener("change", draw);
  update();
  dispose = () => {
    observer.disconnect();
    controls.dispose();
    geometry.dispose();
    grid.dispose();
    Object.values(materials).forEach((material) => material.dispose());
    renderer.dispose();
    renderer.domElement.remove();
  };
});

watch(
  () => [props.recipe, props.streetlights],
  () => update?.(),
);
onBeforeUnmount(() => dispose?.());
</script>

<template>
  <div
    ref="host"
    class="road-preview"
    role="region"
    :aria-label="t('roadAsset.preview')"
  >
    <p>
      {{
        t(unavailable ? "roadAsset.previewUnavailable" : "roadAsset.preview")
      }}
    </p>
  </div>
</template>

<style scoped>
.road-preview {
  position: relative;
  height: 100%;
  overflow: hidden;
  background: var(--surface-elevated);
}
p {
  position: absolute;
  z-index: 1;
  inset-block-start: 8px;
  inset-inline-start: 10px;
  margin: 0;
  padding: 5px 7px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface);
  color: var(--muted-foreground);
  font-size: 11px;
  pointer-events: none;
}
</style>
