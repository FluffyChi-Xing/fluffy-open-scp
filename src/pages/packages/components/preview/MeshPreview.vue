<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import type * as ThreeNamespace from "three";

const props = defineProps<{ objBase64: string }>();
const { t } = useI18n();
const container = shallowRef<HTMLElement | null>(null);
const error = ref("");
const ready = ref(false);
const lightAzimuth = ref(45);
const lightElevation = ref(55);

let THREE: typeof ThreeNamespace | null = null;
let renderer: ThreeNamespace.WebGLRenderer | null = null;
let scene: ThreeNamespace.Scene | null = null;
let camera: ThreeNamespace.PerspectiveCamera | null = null;
let pivot: ThreeNamespace.Group | null = null;
let light: ThreeNamespace.PointLight | null = null;
let grid: ThreeNamespace.GridHelper | null = null;
let frame = 0;
let lightRadius = 5;
let cameraDistance = 5;
let orbitTheta = 0.34;
let orbitPhi = 1.2;
let observer: ResizeObserver | undefined;
let dragging = false;
let lastX = 0;
let lastY = 0;

function decodeBase64(base64: string): Uint8Array<ArrayBuffer> {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}
function disposeObject(object: ThreeNamespace.Object3D) {
  object.traverse((child) => {
    const mesh = child as ThreeNamespace.Mesh;
    if (mesh.isMesh) {
      mesh.geometry?.dispose();
      const material = mesh.material;
      if (Array.isArray(material)) material.forEach((item) => item.dispose());
      else material?.dispose();
    }
  });
}
function setLightPosition() {
  if (!light) return;
  const azimuth = (lightAzimuth.value * Math.PI) / 180;
  const elevation = (lightElevation.value * Math.PI) / 180;
  light.position.set(
    lightRadius * Math.cos(elevation) * Math.sin(azimuth),
    lightRadius * Math.sin(elevation),
    lightRadius * Math.cos(elevation) * Math.cos(azimuth),
  );
}
function applyCamera() {
  // 轨道相机：始终注视模型中心，方位角/俯仰角全自由（极点略内收避免翻转）。
  if (!camera || !THREE) return;
  const phi = Math.min(Math.PI - 0.02, Math.max(0.02, orbitPhi));
  camera.position.setFromSpherical(new THREE.Spherical(cameraDistance, phi, orbitTheta));
  camera.lookAt(0, 0, 0);
}
function renderLoop() {
  frame = requestAnimationFrame(renderLoop);
  renderer?.render(scene!, camera!);
}
async function build() {
  if (!container.value) return;
  ready.value = false;
  error.value = "";
  try {
    THREE = await import("three");
    const { OBJLoader } = await import("three/examples/jsm/loaders/OBJLoader.js");
    const text = new TextDecoder().decode(decodeBase64(props.objBase64));
    const object = new OBJLoader().parse(text);
    const material = new THREE.MeshStandardMaterial({
      color: 0xb8c2cc,
      roughness: 0.55,
      metalness: 0.12,
      side: THREE.DoubleSide,
    });
    object.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (mesh.isMesh) {
        mesh.material = material;
        if (!mesh.geometry.attributes.normal) mesh.geometry.computeVertexNormals();
      }
    });
    // RW4 为 Z-up 坐标系，-90° X 旋转后建筑在 Y-up 视图中站立。
    object.rotation.x = -Math.PI / 2;
    object.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(object);
    const sphere = box.getBoundingSphere(new THREE.Sphere());
    const radius = Math.max(sphere.radius, 1e-3);
    const center = box.getCenter(new THREE.Vector3());
    object.position.sub(center);
    if (pivot) {
      pivot.clear();
      disposeObject(pivot);
      scene?.remove(pivot);
    }
    if (grid) {
      grid.geometry.dispose();
      (grid.material as ThreeNamespace.Material).dispose();
      scene?.remove(grid);
    }
    pivot = new THREE.Group();
    pivot.rotation.set(0, 0, 0);
    pivot.add(object);
    scene!.add(pivot);
    grid = new THREE.GridHelper(radius * 12, 48, 0x8a93a6, 0x565e6e);
    const gridMaterial = grid.material as ThreeNamespace.Material;
    gridMaterial.transparent = true;
    gridMaterial.opacity = 0.32;
    grid.position.y = box.min.y - radius * 0.002;
    scene!.add(grid);
    lightRadius = radius * 2.4;
    cameraDistance = radius * 3;
    const direction = new THREE.Vector3(0.35, 0.4, 1).normalize();
    orbitPhi = Math.acos(THREE.MathUtils.clamp(direction.y, -1, 1));
    orbitTheta = Math.atan2(direction.x, direction.z);
    camera!.near = radius / 100;
    camera!.far = radius * 40;
    camera!.updateProjectionMatrix();
    applyCamera();
    setLightPosition();
    ready.value = true;
  } catch {
    error.value = t("package.meshLoadFailed");
  }
}
function onPointerDown(event: PointerEvent) {
  dragging = true;
  lastX = event.clientX;
  lastY = event.clientY;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}
function onPointerMove(event: PointerEvent) {
  if (!dragging) return;
  // 拖拽调整轨道相机（方位角 + 俯仰角），模型保持居中不旋转。
  orbitTheta -= ((event.clientX - lastX) * Math.PI) / 180 * 0.6;
  orbitPhi -= ((event.clientY - lastY) * Math.PI) / 180 * 0.6;
  applyCamera();
  lastX = event.clientX;
  lastY = event.clientY;
}
function onPointerUp() {
  dragging = false;
}
function onWheel(event: WheelEvent) {
  event.preventDefault();
  if (!scene) return;
  const factor = event.deltaY > 0 ? 1.12 : 1 / 1.12;
  const radius = lightRadius / 2.4;
  cameraDistance = Math.min(
    radius * 12,
    Math.max(radius * 0.3, cameraDistance * factor),
  );
  applyCamera();
}
function resize() {
  const element = container.value;
  if (!element || !renderer || !camera) return;
  const width = element.clientWidth;
  const height = element.clientHeight;
  if (width === 0 || height === 0) return;
  renderer.setSize(width, height);
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
}
onMounted(async () => {
  const element = container.value;
  if (!element) return;
  THREE = await import("three");
  scene = new THREE.Scene();
  camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
  renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(window.devicePixelRatio);
  element.appendChild(renderer.domElement);
  scene.add(new THREE.AmbientLight(0xffffff, 0.38));
  light = new THREE.PointLight(0xffffff, 2.2, 0, 0);
  scene.add(light);
  element.addEventListener("wheel", onWheel, { passive: false });
  observer = new ResizeObserver(resize);
  observer.observe(element);
  resize();
  await build();
  renderLoop();
});
onBeforeUnmount(() => {
  cancelAnimationFrame(frame);
  observer?.disconnect();
  container.value?.removeEventListener("wheel", onWheel);
  if (pivot) disposeObject(pivot);
  grid?.geometry.dispose();
  (grid?.material as ThreeNamespace.Material | null)?.dispose();
  light?.dispose();
  renderer?.dispose();
  renderer?.domElement.remove();
});
watch([lightAzimuth, lightElevation], setLightPosition);
watch(
  () => props.objBase64,
  () => void build(),
);
</script>

<template>
  <div class="mesh-preview">
    <div class="mesh-controls" role="group" :aria-label="$t('package.lightControls')">
      <label>
        <span>{{ $t("package.lightAzimuth") }}</span>
        <input v-model.number="lightAzimuth" type="range" min="0" max="360" />
      </label>
      <label>
        <span>{{ $t("package.lightElevation") }}</span>
        <input v-model.number="lightElevation" type="range" min="5" max="175" />
      </label>
    </div>
    <div
      ref="container"
      class="mesh-viewport"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerUp"
    />
    <p v-if="error" class="mesh-error" role="alert">{{ error }}</p>
  </div>
</template>

<style scoped>
.mesh-preview {
  display: grid;
  gap: 8px;
  min-width: 0;
}
.mesh-controls {
  display: grid;
  gap: 6px;
}
.mesh-controls label {
  align-items: center;
  display: grid;
  font-size: 11px;
  grid-template-columns: auto 160px;
  gap: 8px;
}
.mesh-controls span {
  color: var(--subtle-foreground);
}
.mesh-controls input[type="range"] {
  accent-color: var(--primary);
  width: 160px;
}
.mesh-viewport {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: grab;
  height: 280px;
  min-width: 0;
  overflow: hidden;
  touch-action: none;
}
.mesh-viewport:active {
  cursor: grabbing;
}
.mesh-viewport canvas {
  display: block;
}
.mesh-error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  padding: 0 16px;
}
</style>
