<script setup lang="ts">
import { onMounted, onUnmounted, shallowRef, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import * as THREE from "three";
import FButton from "@/components/ui/FButton.vue";
import { disposeObject } from "@/lib/three-viewer";
import { installHejlToneMapping } from "@/lib/hejlTonemapping";
import type { CameraFrame, CameraProfile } from "./cityCamera";
import { focusPlane } from "./cityCameraFocus";
import { createCityCameraEffects } from "./cityCameraEffects";
import { loadCameraFireStation } from "./cameraFireStation";

const props = defineProps<{
  frame: CameraFrame;
  profile?: CameraProfile;
  playing: boolean;
  motionStrength: number;
  packageId?: number;
}>();
const emit = defineEmits<{
  tilt: [delta: number];
  zoom: [step: number];
  reset: [];
}>();
const { t } = useI18n();
const canvas = useTemplateRef<HTMLCanvasElement>("canvas");
const state = shallowRef<"loading" | "ready" | "webgl" | "failure">("loading");
const dragging = shallowRef(false);
let cleanup: (() => void) | undefined;
let render: (() => void) | undefined;
let active: { id: number; x: number; y: number; pan: boolean } | undefined;
let yaw = 0;
let wheelDelta = 0;
const pan = new THREE.Vector3();
let panBy: ((dx: number, dy: number) => void) | undefined;

function startDrag(event: PointerEvent) {
  if (state.value !== "ready" || active || event.button > 2) return;
  event.preventDefault();
  canvas.value?.focus();
  canvas.value?.setPointerCapture(event.pointerId);
  active = {
    id: event.pointerId,
    x: event.clientX,
    y: event.clientY,
    pan: event.button !== 0 || event.shiftKey,
  };
  dragging.value = true;
}

function moveDrag(event: PointerEvent) {
  if (!active || active.id !== event.pointerId) return;
  const dx = event.clientX - active.x;
  const dy = event.clientY - active.y;
  active.x = event.clientX;
  active.y = event.clientY;
  if (active.pan) panBy?.(dx, dy);
  else {
    yaw -= dx * 0.006;
    emit("tilt", dy * 0.004);
  }
  render?.();
}

function stopDrag(event: PointerEvent) {
  if (active?.id !== event.pointerId) return;
  active = undefined;
  dragging.value = false;
  render?.();
}

function wheel(event: WheelEvent) {
  if (state.value !== "ready" || event.ctrlKey || !event.deltaY) return;
  event.preventDefault();
  if (event.deltaMode !== WheelEvent.DOM_DELTA_PIXEL) {
    emit("zoom", Math.sign(event.deltaY));
    return;
  }
  if (Math.sign(wheelDelta) !== Math.sign(event.deltaY)) wheelDelta = 0;
  wheelDelta += event.deltaY;
  if (Math.abs(wheelDelta) < 48) return;
  emit("zoom", Math.sign(wheelDelta));
  wheelDelta = 0;
}

function resetOffsets() {
  pan.set(0, 0, 0);
  yaw = 0;
  wheelDelta = 0;
}
function resetView() {
  resetOffsets();
  emit("reset");
  render?.();
}

function handleKey(event: KeyboardEvent) {
  if (event.altKey || event.ctrlKey || event.metaKey) return;
  switch (event.key) {
    case "ArrowLeft":
      yaw += 0.1;
      break;
    case "ArrowRight":
      yaw -= 0.1;
      break;
    case "ArrowUp":
      emit("tilt", -0.04);
      break;
    case "ArrowDown":
      emit("tilt", 0.04);
      break;
    case "+":
    case "=":
      emit("zoom", -1);
      break;
    case "-":
      emit("zoom", 1);
      break;
    case "Home":
      resetView();
      break;
    default:
      return;
  }
  event.preventDefault();
  render?.();
}

onMounted(() => {
  const target = canvas.value!;
  let renderer: THREE.WebGLRenderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas: target, antialias: true });
  } catch {
    state.value = "webgl";
    return;
  }
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
  installHejlToneMapping(THREE);
  renderer.toneMapping = THREE.CustomToneMapping;
  renderer.toneMappingExposure = 1.12;
  renderer.shadowMap.type = THREE.PCFShadowMap;

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x9dbaca);
  scene.add(new THREE.HemisphereLight(0xe6f5ff, 0x86826c, 0.65));
  const sun = new THREE.DirectionalLight(0xffe0b3, 2.2);
  sun.castShadow = true;
  sun.shadow.camera.left = sun.shadow.camera.bottom = -54;
  sun.shadow.camera.right = sun.shadow.camera.top = 54;
  sun.shadow.mapSize.set(1024, 1024);
  sun.shadow.normalBias = 0.08;
  scene.add(sun, sun.target);
  const world = new THREE.Group();
  // The shared LOTM and lot stages are Z-up, the camera frames are Y-up.
  world.rotation.x = -Math.PI / 2;
  scene.add(world);

  const camera = new THREE.PerspectiveCamera();
  const effects = createCityCameraEffects(renderer);
  const previous = new THREE.Matrix4();
  const lastViewProjection = new THREE.Matrix4();
  const yAxis = new THREE.Vector3(0, 1, 0);
  const pivot = new THREE.Vector3();
  const right = new THREE.Vector3();
  const forward = new THREE.Vector3();
  let anchor: Awaited<ReturnType<typeof loadCameraFireStation>> = null;
  let animation = 0;
  let paint = 0;
  let lastWidth = 0;
  let lastHeight = 0;
  let started: number | undefined;
  let disposed = false;

  panBy = (dx, dy) => {
    right.setFromMatrixColumn(camera.matrixWorld, 0).setY(0).normalize();
    forward.crossVectors(yAxis, right);
    const metresPerPixel =
      (2 * props.frame.distance * Math.tan((camera.fov * Math.PI) / 360)) /
      Math.max(1, target.clientHeight);
    pan.addScaledVector(right, -dx * metresPerPixel);
    pan.addScaledVector(forward, -dy * metresPerPixel);
  };

  const pose = (elapsed: number, rotating: boolean) => {
    if (!anchor) return;
    const turn = rotating
      ? (Math.sin((elapsed * Math.PI) / 1.2) * Math.PI) / 9
      : 0;
    const { eye, target: lookAt } = props.frame;
    const rise = eye[1] - lookAt[1];
    const eyeHeight = Math.max(
      anchor.groundHeight + 1.7,
      anchor.center.y + rise,
    );
    pivot.copy(anchor.center).add(pan);
    pivot.y = eyeHeight - rise;
    camera.position
      .set(eye[0], rise, eye[2])
      .applyAxisAngle(yAxis, yaw + turn)
      .add(pivot);
    camera.lookAt(pivot);
    camera.updateMatrixWorld();
  };

  const draw = (elapsed: number, rotating: boolean) => {
    if (!anchor) return;
    const { fov, near, far, pitch } = props.frame;
    Object.assign(camera, {
      fov,
      near,
      far,
      aspect: target.clientWidth / target.clientHeight,
    });
    camera.updateProjectionMatrix();
    if (rotating) {
      pose(elapsed - 1 / 30, true);
      previous.multiplyMatrices(
        camera.projectionMatrix,
        camera.matrixWorldInverse,
      );
    } else {
      previous.copy(lastViewProjection);
    }
    pose(elapsed, rotating);
    lastViewProjection.multiplyMatrices(
      camera.projectionMatrix,
      camera.matrixWorldInverse,
    );
    const focus = focusPlane(
      props.profile,
      camera.position.y - anchor.groundHeight,
      pitch,
      fov,
      camera.aspect,
    );
    effects.render(
      scene,
      camera,
      focus,
      previous,
      rotating || active ? props.motionStrength : null,
    );
  };

  const frame = (now: number) => {
    if (!props.playing || !anchor) return;
    started ??= now;
    if (target.clientWidth && target.clientHeight)
      draw((now - started) / 1000, true);
    animation = requestAnimationFrame(frame);
  };

  render = () => {
    const { clientWidth: width, clientHeight: height } = target;
    if (!width || !height || !anchor) return;
    if (width !== lastWidth || height !== lastHeight) {
      renderer.setSize(width, height, false);
      lastWidth = width;
      lastHeight = height;
    }
    if (props.playing) {
      if (!animation) animation = requestAnimationFrame(frame);
    } else if (!paint) {
      paint = requestAnimationFrame(() => {
        paint = 0;
        if (!props.playing && target.clientWidth && target.clientHeight)
          draw(0, false);
      });
    }
  };
  const observer = new ResizeObserver(render);
  observer.observe(target);
  cleanup = () => {
    disposed = true;
    observer.disconnect();
    cancelAnimationFrame(animation);
    cancelAnimationFrame(paint);
    effects.dispose();
    disposeObject(world);
    sun.shadow.dispose();
    renderer.dispose();
  };

  void loadCameraFireStation(
    props.packageId,
    renderer,
    world,
    sun,
    () => disposed,
  )
    .then((loaded) => {
      if (!loaded || disposed) return;
      anchor = loaded;
      sun.target.position.copy(anchor.center);
      sun.position.add(anchor.center);
      state.value = "ready";
      render?.();
    })
    .catch((error: unknown) => {
      if (disposed) return;
      console.warn("[city-camera] Fire station scene unavailable", error);
      state.value = "failure";
    });

  watch(
    () => props.playing,
    (playing) => {
      if (!playing) {
        cancelAnimationFrame(animation);
        animation = 0;
        started = undefined;
      }
      render?.();
    },
  );
});

watch(
  () => [props.frame, props.profile],
  () => render?.(),
);
watch(() => props.profile?.instance, resetOffsets);
onUnmounted(() => cleanup?.());
</script>

<template>
  <div class="camera-container">
    <canvas
      ref="canvas"
      class="viewport"
      :class="{ dragging }"
      tabindex="0"
      :aria-label="t('studio.camera.fireStation')"
      :aria-describedby="
        state === 'ready' ? 'city-camera-pointer-help' : undefined
      "
      :aria-hidden="state !== 'ready'"
      :inert="state !== 'ready'"
      @pointerdown="startDrag"
      @pointermove="moveDrag"
      @pointerup="stopDrag"
      @pointercancel="stopDrag"
      @lostpointercapture="stopDrag"
      @contextmenu.prevent
      @wheel="wheel"
      @keydown="handleKey"
    />
    <p v-if="state !== 'ready'" class="unavailable" role="status">
      {{
        t(
          state === "webgl"
            ? "studio.camera.webgl"
            : state === "loading"
              ? "studio.camera.fireStationLoading"
              : "studio.camera.fireStationFailed",
        )
      }}
    </p>
    <div v-else class="viewport-footer">
      <p id="city-camera-pointer-help">{{ t("studio.camera.pointerHelp") }}</p>
      <FButton variant="secondary" @click="resetView">{{
        t("studio.camera.resetView")
      }}</FButton>
    </div>
  </div>
</template>

<style scoped>
.camera-container {
  position: relative;
}
.viewport {
  display: block;
  width: 100%;
  height: clamp(320px, 46vh, 640px);
  background: var(--background);
  cursor: grab;
  touch-action: none;
}
.viewport.dragging {
  cursor: grabbing;
}
.viewport:focus-visible {
  outline: 2px solid var(--brand);
  outline-offset: -2px;
}
.unavailable {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  margin: 0;
  padding: 18px;
  text-align: center;
  color: var(--muted-foreground);
  pointer-events: none;
}
.viewport-footer {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  border-bottom: 1px solid var(--border);
}
.viewport-footer p {
  margin: 0;
  font-size: 12px;
  color: var(--muted-foreground);
}
</style>
