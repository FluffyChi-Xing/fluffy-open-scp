<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { ThreeViewer, disposeObject } from "@/lib/three-viewer";
import { parseLotModelObjects, pngBlobUrl } from "@/lib/three-gltf";
import type * as ThreeNamespace from "three";
import type { LotModelPayload, LotUnitDto } from "@/api/tauri";
import type { ModelState, UnitGrouping } from "./usePropertyEditorSession";
import {
  buildPathLine,
  buildRealLightUnit,
  buildUnitObject,
  unitId,
} from "./unitGizmos";

const props = defineProps<{
  modelPayload: LotModelPayload | null;
  renderMode: "default" | "refined";
  grouping: UnitGrouping;
  lotSize: [number, number] | null;
  /** LotPlacementTransform 行主序 12 floats；地面矩形取其逆对齐建筑。 */
  lotPlacement: number[] | null;
  lotMaskPng: string | null;
  selectedId: string | null;
  hiddenUnits: Set<string>;
  groupVisibility: Record<string, boolean>;
  modelState: ModelState;
}>();
const emit = defineEmits<{
  select: [id: string | null];
  "toggle-layer": [name: string];
}>();
useI18n();
const container = shallowRef<HTMLElement | null>(null);
const viewer = shallowRef<ThreeViewer | null>(null);
const sceneReady = ref(false);
const lightPanelOpen = ref(false);
const lightAzimuth = ref(45);
const lightElevation = ref(55);
/** 日/夜模拟：全局环境亮度倍率（1 = 当前观感，0 ≈ 夜，2 ≈ 正午）。 */
const brightness = ref(1);

watch([lightAzimuth, lightElevation], () => {
  viewer.value?.setKeyLight(lightAzimuth.value, lightElevation.value);
});
watch(brightness, () => applyBrightness());

const GROUP_KEYS = [
  "model",
  "lights",
  "props",
  "decals",
  "effects",
  "spawners",
  "paths",
] as const;

const unitObjects = new Map<string, ThreeNamespace.Object3D>();
/** 当前 payload 派生的贴图 blob URL；rebuild 时回收上一代。 */
const textureUrls: string[] = [];
/**
 * 场景真实光源总数上限：WebGL 前向渲染每个片元都评估全部光源，
 * 多灯地块（路灯密集的建筑群）推近镜头时片元数×光源数导致掉帧。
 * 超限时从整体强度最弱的单元开始摘除真实光源（保留透明拾取代理）。
 */
const MAX_REAL_LIGHTS = 24;
let rebuildToken = 0;

onMounted(async () => {
  const element = container.value;
  if (!element) return;
  viewer.value = await ThreeViewer.create(element, {
    onTap: (hit) => {
      let node: ThreeNamespace.Object3D | null = hit.object;
      while (node) {
        if (typeof node.userData?.unitId === "string") {
          emit("select", node.userData.unitId);
          return;
        }
        node = node.parent;
      }
      emit("select", null);
    },
  });
  await rebuild();
});
onBeforeUnmount(() => {
  rebuildToken += 1;
  viewer.value?.dispose();
  viewer.value = null;
  unitObjects.clear();
  for (const url of textureUrls) URL.revokeObjectURL(url);
  textureUrls.length = 0;
});

function kindGroup(kind: LotUnitDto["kind"]) {
  return kind === "pathPoint" ? "paths" : `${kind}s`;
}

async function rebuild() {
  const instance = viewer.value;
  if (!instance) return;
  const token = ++rebuildToken;
  const THREE = instance.THREE;
  for (const name of GROUP_KEYS) instance.clearGroup(name);
  unitObjects.clear();
  for (const url of textureUrls) URL.revokeObjectURL(url);
  textureUrls.length = 0;

  const payload = props.modelPayload;
  const modelObjects = payload ? await parseLotModelObjects(payload.glbs) : [];
  if (token !== rebuildToken) {
    for (const object of modelObjects) disposeObject(object);
    return;
  }
  const whiteMaterial = new THREE.MeshStandardMaterial({
    color: 0xb8c2cc,
    roughness: 0.55,
    metalness: 0.12,
    side: THREE.DoubleSide,
  });
  const refinedMaterials: ThreeNamespace.MeshStandardMaterial[] = [];
  for (const object of modelObjects) {
    object.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (!mesh.isMesh) return;
      if (props.renderMode !== "refined") {
        mesh.material = whiteMaterial;
        return;
      }
      // GLB 的 COLOR_0（调色板顶点色）→ GLTFLoader 的 color 顶点属性
      const refined = new THREE.MeshStandardMaterial({
        vertexColors: Boolean(mesh.geometry.attributes.color),
        roughness: 0.82,
        metalness: 0,
        side: THREE.DoubleSide,
      });
      mesh.material = refined;
      refinedMaterials.push(refined);
    });
    instance.group("model").add(object);
  }
  // 精细贴图：区域遮罩红通道 baseColor + 解 Swizzle 法线（仅 FLOAT2 UV 模型，服务端判定）
  if (props.renderMode === "refined" && payload?.hasUv) {
    const generation = token;
    const loader = new THREE.TextureLoader();
    const loadTexture = (
      bytes: Uint8Array<ArrayBuffer>,
      setup: (texture: ThreeNamespace.Texture) => void,
    ) => {
      const url = pngBlobUrl(bytes);
      textureUrls.push(url);
      loader.load(
        url,
        (texture) => {
          if (generation !== rebuildToken) {
            texture.dispose();
            return;
          }
          setup(texture);
          for (const material of refinedMaterials) {
            material.needsUpdate = true;
          }
        },
        undefined,
        () => {},
      );
    };
    if (payload.baseColorPng) {
      loadTexture(payload.baseColorPng, (texture) => {
        texture.colorSpace = THREE.SRGBColorSpace;
        for (const material of refinedMaterials) {
          material.map = texture;
        }
      });
    }
    if (payload.normalPng) {
      loadTexture(payload.normalPng, (texture) => {
        for (const material of refinedMaterials) {
          material.normalMap = texture;
        }
      });
    }
  }

  // Lot 地面矩形（LotSize）；有 LotMask 时异步贴四色量化图。
  if (props.lotSize) {
    const ground = buildLotRect(THREE, props.lotSize);
    // C# CreateLotModel：地面按 LotPlacementTransform 的逆矩阵摆放——
    // 建筑在地块内不居中时，逆变换把遮罩图案对回建筑原点。
    if (props.lotPlacement) {
      const m = props.lotPlacement;
      const inverse = new THREE.Matrix4()
        .set(
          m[0], m[3], m[6], m[9],
          m[1], m[4], m[7], m[10],
          m[2], m[5], m[8], m[11],
          0, 0, 0, 1,
        )
        .invert();
      ground.matrixAutoUpdate = false;
      ground.matrix.copy(inverse);
    }
    instance.group("model").add(ground);
    if (props.lotMaskPng) {
      const generation = token;
      new THREE.TextureLoader().load(props.lotMaskPng, (texture) => {
        if (generation !== rebuildToken) {
          texture.dispose();
          return;
        }
        texture.colorSpace = THREE.SRGBColorSpace;
        const fill = ground.children.find((child) => (child as ThreeNamespace.Mesh).isMesh) as
          | ThreeNamespace.Mesh
          | undefined;
        if (fill) {
          const material = fill.material as ThreeNamespace.MeshBasicMaterial;
          material.map = texture;
          material.transparent = false;
          material.opacity = 1;
          material.color.set(0xffffff);
          material.needsUpdate = true;
        }
      });
    }
  }

  const units: LotUnitDto[] = [
    ...props.grouping.lights,
    ...props.grouping.decals,
    ...props.grouping.props,
    ...props.grouping.effects,
    ...props.grouping.spawners,
    ...props.grouping.pathPoints,
  ];
  for (const unit of units) {
    // 精细模式：光源用真实 three.js 光源；其余组件保持标记锥
    const object =
      props.renderMode === "refined" && unit.kind === "light"
        ? buildRealLightUnit(THREE, unit)
        : buildUnitObject(THREE, unit);
    if (!object) continue;
    instance.group(kindGroup(unit.kind)).add(object);
    unitObjects.set(unitId(unit), object);
  }

  // 路径折线：按 point_index 排序连接（pathPairs 语义未定，先 best-effort）。
  const points = [...props.grouping.pathPoints]
    .filter((point) => point.point)
    .sort(
      (a, b) => (a.pointIndex ?? a.index) - (b.pointIndex ?? b.index),
    )
    .map((point) => new THREE.Vector3(...point.point!));
  const line = buildPathLine(THREE, points);
  if (line) instance.group("paths").add(line);

  pruneExcessLights(instance);

  instance.frameContent();
  instance.setKeyLight(45, 55);
  applyBrightness();
  applyGroupVisibility();
  applyUnitVisibility();
  applySelection();
  sceneReady.value = true;
}

/** 日/夜亮度：仅缩放环境四灯；地块真实光源保持常亮（夜间灯依然亮）。 */
function applyBrightness() {
  viewer.value?.setEnvironmentBrightness(brightness.value);
}

/** 真实光源总数超限时，从强度最弱的单元开始摘除光源本体。 */
function pruneExcessLights(instance: ThreeViewer) {
  const perUnit: {
    lights: ThreeNamespace.Light[];
    intensity: number;
  }[] = [];
  let total = 0;
  for (const [id, object] of unitObjects) {
    if (!id.startsWith("light:")) continue;
    const lights: ThreeNamespace.Light[] = [];
    object.traverse((child) => {
      if ((child as ThreeNamespace.Light).isLight) {
        lights.push(child as ThreeNamespace.Light);
      }
    });
    if (!lights.length) continue;
    total += lights.length;
    perUnit.push({
      lights,
      intensity: lights.reduce((sum, light) => sum + light.intensity, 0),
    });
  }
  if (total <= MAX_REAL_LIGHTS) return;
  perUnit.sort((a, b) => a.intensity - b.intensity);
  for (const entry of perUnit) {
    for (const light of entry.lights) {
      if (total <= MAX_REAL_LIGHTS) return;
      light.parent?.remove(light);
      light.dispose();
      total -= 1;
    }
  }
}

/** Lot 地面矩形：XY 平面（Z-up 贴地面）细边框 + 半透明填充。 */
function buildLotRect(
  THREE: typeof ThreeNamespace,
  lotSize: [number, number],
): ThreeNamespace.Object3D {
  const [width, depth] = lotSize;
  const group = new THREE.Group();
  const half = [width / 2, depth / 2];
  const corners = [
    new THREE.Vector3(-half[0], -half[1], 0.05),
    new THREE.Vector3(half[0], -half[1], 0.05),
    new THREE.Vector3(half[0], half[1], 0.05),
    new THREE.Vector3(-half[0], half[1], 0.05),
  ];
  const border = new THREE.LineLoop(
    new THREE.BufferGeometry().setFromPoints(corners),
    new THREE.LineBasicMaterial({ color: 0x9aa4b8 }),
  );
  const fill = new THREE.Mesh(
    new THREE.PlaneGeometry(width, depth),
    new THREE.MeshBasicMaterial({
      color: 0x6b7689,
      transparent: true,
      opacity: 0.08,
      side: THREE.DoubleSide,
    }),
  );
  fill.position.z = 0.02;
  group.add(border, fill);
  return group;
}

function applyGroupVisibility() {
  const instance = viewer.value;
  if (!instance) return;
  for (const name of GROUP_KEYS) {
    instance.group(name).visible = props.groupVisibility[name] !== false;
  }
}
function applyUnitVisibility() {
  for (const [id, object] of unitObjects) {
    object.visible = !props.hiddenUnits.has(id);
  }
}
function applySelection() {
  const selected = props.selectedId
    ? unitObjects.get(props.selectedId) ?? null
    : null;
  viewer.value?.setSelected(selected);
}

watch(
  () => [props.modelPayload, props.renderMode, props.grouping],
  () => void rebuild(),
);
watch(() => props.groupVisibility, applyGroupVisibility, { deep: true });
watch(() => props.hiddenUnits, applyUnitVisibility);
watch(() => props.selectedId, applySelection);
</script>

<template>
  <div class="viewport-pane">
    <div ref="container" class="viewport-3d" />
    <div v-if="modelState === 'loading'" class="viewport-overlay viewport-status">
      <FSpinner size="sm" :label="$t('common.loading')" />
    </div>
    <p
      v-else-if="modelState === 'missing' || modelState === 'error'"
      class="viewport-overlay viewport-status"
      role="status"
    >
      {{
        modelState === "missing"
          ? $t("package.propertyEditorModelMissing")
          : $t("package.propertyEditorModelError")
      }}
    </p>
    <div class="viewport-overlay viewport-tools">
      <button
        type="button"
        :aria-label="$t('package.lightControls')"
        :title="$t('package.lightControls')"
        :aria-pressed="lightPanelOpen"
        :class="{ active: lightPanelOpen }"
        @click="lightPanelOpen = !lightPanelOpen"
      >
        <FIcon name="Lightbulb" :size="13" aria-label="" />
      </button>
      <button
        type="button"
        :aria-label="$t('package.resetView')"
        :title="$t('package.resetView')"
        @click="viewer?.resetView()"
      >
        <FIcon name="RotateCcw" :size="13" aria-label="" />
      </button>
    </div>
    <div v-if="lightPanelOpen" class="viewport-overlay viewport-light-panel">
      <label>
        <span>{{ $t("package.lightAzimuth") }}</span>
        <input v-model.number="lightAzimuth" type="range" min="0" max="360" />
      </label>
      <label>
        <span>{{ $t("package.lightElevation") }}</span>
        <input v-model.number="lightElevation" type="range" min="5" max="175" />
      </label>
      <label>
        <span>{{ $t("package.lightBrightness") }}</span>
        <input v-model.number="brightness" type="range" min="0" max="2" step="0.05" />
      </label>
    </div>
    <div class="viewport-overlay viewport-visibility" role="group" :aria-label="$t('package.visibilityToggles')">
      <button
        v-for="name in GROUP_KEYS"
        :key="name"
        type="button"
        :class="{ off: groupVisibility[name] === false }"
        :aria-pressed="groupVisibility[name] !== false"
        :title="$t(`package.group${name[0].toUpperCase()}${name.slice(1)}`)"
        @click="$emit('toggle-layer', name)"
      >
        <FIcon :name="groupVisibility[name] === false ? 'EyeOff' : 'Eye'" :size="13" aria-label="" />
        <span>{{ $t(`package.group${name[0].toUpperCase()}${name.slice(1)}`) }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.viewport-pane {
  background: var(--surface-elevated);
  display: grid;
  min-height: 0;
  min-width: 0;
  position: relative;
}
.viewport-3d {
  cursor: grab;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  touch-action: none;
}
.viewport-3d:active {
  cursor: grabbing;
}
.viewport-3d canvas {
  display: block;
}
.viewport-overlay {
  position: absolute;
  z-index: 1;
}
.viewport-status {
  align-self: center;
  justify-self: center;
  background: color-mix(in srgb, var(--surface) 82%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: grid;
  font-size: 12px;
  padding: 8px 14px;
  pointer-events: none;
}
.viewport-tools {
  display: flex;
  gap: 4px;
  right: 10px;
  top: 10px;
}
.viewport-tools button {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 28px;
  min-width: 28px;
}
.viewport-tools button:hover,
.viewport-tools button.active {
  color: var(--foreground);
}
.viewport-tools button.active {
  background: var(--accent);
}
.viewport-light-panel {
  background: color-mix(in srgb, var(--surface) 92%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: grid;
  gap: 6px;
  padding: 10px;
  right: 10px;
  top: 48px;
}
.viewport-light-panel label {
  align-items: center;
  display: grid;
  font-size: 11px;
  grid-template-columns: auto 140px;
  gap: 8px;
}
.viewport-light-panel span {
  color: var(--subtle-foreground);
}
.viewport-light-panel input[type="range"] {
  accent-color: var(--primary);
  width: 140px;
}
.viewport-visibility {
  display: grid;
  gap: 3px;
  left: 10px;
  top: 10px;
}
.viewport-visibility button {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 5px;
  justify-content: flex-start;
  min-height: 24px;
  padding: 0 8px;
}
.viewport-visibility button.off {
  color: var(--subtle-foreground);
  opacity: 0.7;
}
</style>
