<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { disposeObject } from "@/lib/three-viewer";
import { parseLotModelObjects } from "@/lib/three-gltf";
import type * as ThreeNamespace from "three";
import type { LotModelLodRef, LotModelPayload, LotUnitDto } from "@/api/tauri";
import type { ModelState, UnitGrouping } from "./usePropertyEditorSession";
import {
  buildPathLine,
  buildRealLightUnit,
  buildUnitObject,
  unitId,
} from "./unitGizmos";
import { useEditorViewport } from "./useEditorViewport";
import {
  applyDeferredMaterialMaps,
  applySunEnv,
  createSunEnv,
  dayFactor,
  loadTintTextures,
  makeTintMaterial,
  type SunEnvRefs,
} from "./refinedRender";
import { threeToRowMajor } from "./unitEditLayer";
import type { TransformControls } from "three/examples/jsm/controls/TransformControls.js";
import {
  applyGroundMask,
  buildLotRect,
  placementInverse,
} from "./editorGround";

export type EditorTool = "select" | "translate" | "rotate" | "scale";

/**
 * Lot 场景装配层（业务形态相关）：在 useEditorViewport 底座上组合
 * refinedRender（tint shader/日夜环境）、editorGround（地面）与
 * unitGizmos（六类 Unit）完成 SCP lot 视口。浮层 UI 亦在此层。
 * 底座/业务拆分见 docs/roadmap/asset-development.md §二（PE-重构-1）。
 */
const props = defineProps<{
  modelPayload: LotModelPayload | null;
  /** LOD1~LOD4 资源位置（index 0 = LOD1）；缺失级为 null。 */
  modelLods: (LotModelLodRef | null)[];
  activeLod: number;
  renderMode: "default" | "refined";
  grouping: UnitGrouping;
  lotSize: [number, number] | null;
  /** LotPlacementTransform 行主序 12 floats；地面矩形取其逆对齐建筑。 */
  lotPlacement: number[] | null;
  /** LotColor1-4 RGBA（A = 地面贴图索引 0-15）。 */
  lotColors: [number, number, number, number][];
  /** LotColor1-4 是否实际存在（false = 回退色，不参与着色）。 */
  lotColorsAuthored: boolean[];
  lotMaskPng: string | null;
  /** LotMask 原始通道权重图（v4 软混合输入）。 */
  /** LotMask 原始通道权重（未压缩 RGBA base64；A = LC4 权重）。 */
  lotMaskRawRgba: string | null;
  /** 默认模式地表反照率（通道平色+底图格；缺失时回退量化图）。 */
  lotAlbedoPng: string | null;
  /** "Lot Textures" 地表共享纹理（data URL；精细模式地面 v2 用）。 */
  lotSurfacePng: string | null;
  selectedId: string | null;
  hiddenUnits: Set<string>;
  groupVisibility: Record<string, boolean>;
  modelState: ModelState;
  /** 通道实验开关（仅精细模式显示）；关闭时恒用自动逐像素。 */
  specExperiment?: boolean;
  /** 0=自动逐像素（默认）/ 1=强制 G·墙 / 2=强制 B·窗（源码字面）。 */
  specMode?: number;
  /** 日/夜时段 0–24（默认 12 正午）；驱动太阳方向/颜色/天空/内景夜灯。 */
  timeOfDay?: number;
  /** 供电（默认 true）：断电 = 内景自发光全灭（源码 interiorThresholds.z hack）。 */
  powered?: boolean;
  /** 当前编辑工具（select = 仅拾取；其余挂 TransformControls 手柄）。 */
  tool?: EditorTool;
}>();
const emit = defineEmits<{
  select: [id: string | null];
  "toggle-layer": [name: string];
  "switch-lod": [index: number];
  "select-tool": [tool: EditorTool];
  /** 手柄拖拽结束提交变换（行主序 12 floats），由壳落本地编辑命令。 */
  "commit-transform": [id: string, matrix: number[]];
  /** 拖拽中的实时变换（id 为 null 表示结束）；坐标面板即时显示用。 */
  "live-transform": [
    id: string | null,
    value: {
      position: [number, number, number];
      rotation: [number, number, number];
      scale: [number, number, number];
    } | null,
  ];
}>();
useI18n();
const viewport = useEditorViewport({
  onTapUnit: (id) => emit("select", id),
});
const lightPanelOpen = ref(false);
const lodPanelOpen = ref(false);
const infoPanelOpen = ref(false);
const infoCopied = ref(false);
let infoCopiedTimer: ReturnType<typeof setTimeout> | undefined;
const lightAzimuth = ref(45);
const lightElevation = ref(55);
/** 日/夜模拟：全局环境亮度倍率（1 = 当前观感，0 ≈ 夜，2 = 正午）。 */
const brightness = ref(1);

watch([lightAzimuth, lightElevation], () => {
  viewport.viewer.value?.setKeyLight(lightAzimuth.value, lightElevation.value);
});
watch(brightness, () => applyBrightness());

/** 存活 tint 材质的 uSpecMode uniform 引用（通道实验热切换，免重建）。 */
const specUniformRefs: { value: number }[] = [];

let envRefs: SunEnvRefs | null = null;

const timeOfDay = () => props.timeOfDay ?? 12;

function applySun() {
  if (envRefs) applySunEnv(envRefs, timeOfDay(), props.powered);
}
watch([() => props.specExperiment, () => props.specMode], () => {
  for (const uniform of specUniformRefs) uniform.value = 2;
});
watch([() => props.timeOfDay, () => props.powered], () => {
  applySun();
  applyBrightness();
});

/** 复制模型槽位诊断（mesh/material/texture 及来源包关系），供复盘。 */
async function copyDiagnostics() {
  const text = props.modelPayload?.diagnostics ?? "";
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // 剪贴板 API 不可用（非安全上下文等）时的兜底
    const helper = document.createElement("textarea");
    helper.value = text;
    document.body.appendChild(helper);
    helper.select();
    document.execCommand("copy");
    helper.remove();
  }
  infoCopied.value = true;
  clearTimeout(infoCopiedTimer);
  infoCopiedTimer = setTimeout(() => {
    infoCopied.value = false;
  }, 1600);
}

function kindGroup(kind: LotUnitDto["kind"]) {
  return kind === "pathPoint" ? "paths" : `${kind}s`;
}

/** TransformControls 手柄（PE-重构-3）：拖拽期间挂起轨道相机，
 * 抬手提交一次行主序矩阵命令。helper 加入 scene 根（相机空间朝向）。 */
let gizmo: TransformControls | null = null;
let dragStartMatrix: ThreeNamespace.Matrix4 | null = null;
/** 默认主灯方位只初始化一次（用户调节后 rebuild 不再重置）。 */
let keyLightInitialized = false;
/** 上一次 rebuild 的模型载荷（身份变化 = 模型/LOD 更换 → 重构图）。 */
let lastPayload: LotModelPayload | null | undefined;

function emitLiveTransform(object: ThreeNamespace.Object3D | null) {
  if (!object || typeof object.userData?.unitId !== "string") {
    emit("live-transform", null, null);
    return;
  }
  const THREE = viewport.viewer.value?.THREE;
  if (!THREE) return;
  const position = new THREE.Vector3();
  const quaternion = new THREE.Quaternion();
  const scale = new THREE.Vector3();
  new THREE.Matrix4()
    .compose(object.position, object.quaternion, object.scale)
    .decompose(position, quaternion, scale);
  const euler = new THREE.Euler().setFromQuaternion(quaternion, "XYZ");
  const deg = 180 / Math.PI;
  emit("live-transform", object.userData.unitId as string, {
    position: [position.x, position.y, position.z],
    rotation: [euler.x * deg, euler.y * deg, euler.z * deg],
    scale: [scale.x, scale.y, scale.z],
  });
}

async function ensureGizmo() {
  const instance = viewport.viewer.value;
  if (!instance || gizmo) return;
  const { TransformControls: Controls } = await import(
    "three/examples/jsm/controls/TransformControls.js"
  );
  if (viewport.viewer.value !== instance) return;
  const controls = new Controls(instance.camera, instance.domElement);
  controls.size = 0.85;
  controls.addEventListener("objectChange", () => {
    if (controls.dragging) emitLiveTransform(controls.object);
  });
  controls.addEventListener("dragging-changed", (event) => {
    const dragging = (event as unknown as { value: boolean }).value;
    instance.setOrbitEnabled(!dragging);
    if (dragging) {
      const object = controls.object;
      dragStartMatrix = object
        ? new instance.THREE.Matrix4().compose(
            object.position,
            object.quaternion,
            object.scale,
          )
        : null;
    } else {
      commitGizmo(controls);
      dragStartMatrix = null;
      emit("live-transform", null, null);
    }
  });
  instance.scene.add(controls.getHelper());
  gizmo = controls;
  updateGizmo();
}

function commitGizmo(controls: TransformControls) {
  const object = controls.object;
  if (!object || typeof object.userData?.unitId !== "string") return;
  const THREE = viewport.viewer.value?.THREE;
  if (!THREE) return;
  const matrix = new THREE.Matrix4().compose(
    object.position,
    object.quaternion,
    object.scale,
  );
  // 未产生位移的点击（拖拽起止矩阵相同）不产生冗余命令
  if (dragStartMatrix && matrix.equals(dragStartMatrix)) return;
  emit("commit-transform", object.userData.unitId as string, threeToRowMajor(matrix));
}

function updateGizmo() {
  const controls = gizmo;
  if (!controls) return;
  controls.detach();
  const tool = props.tool ?? "select";
  if (tool === "select" || !props.selectedId) return;
  const object = viewport.unitObjects.get(props.selectedId);
  if (!object) return;
  controls.setMode(tool);
  controls.attach(object);
}

/** rebuild 包装：模型载荷身份变化时重新构图（编辑操作保持镜头）。 */
function rebuildScene() {
  const reframe = props.modelPayload !== lastPayload;
  lastPayload = props.modelPayload;
  return viewport.rebuild(assembleScene, { reframe });
}

/** 量化 mask 图的尺寸（raw RGBA 字节流构造 ImageData 时需要宽高）。 */
async function loadMaskImageDims(): Promise<{ width: number; height: number } | null> {
  const url = props.lotMaskPng ?? props.lotAlbedoPng;
  if (!url) return null;
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const element = new Image();
      element.onload = () => resolve(element);
      element.onerror = () => reject(new Error("lot mask failed"));
      element.src = url;
    });
    return { width: image.width, height: image.height };
  } catch {
    return null;
  }
}

/** LotMask 原始通道权重图 → ImageData（compose 输入）。
 *  后端是未压缩 RGBA 字节流 base64（A = LC4 权重），**必须用 atob 直接构造
 *  ImageData**——若经 canvas 解码，预乘 alpha 会按 LC4 权重等比压缩/清零
 *  LC1-3 权重，精细合成随即满地判为草皮（2026-09-13 消防局对拍根因）。 */
function loadRawMaskPixels(width: number, height: number): ImageData | null {
  const base64 = props.lotMaskRawRgba;
  if (!base64) return null;
  const binary = atob(base64);
  const expected = width * height * 4;
  if (binary.length < expected) return null;
  const pixels = new Uint8ClampedArray(expected);
  for (let index = 0; index < expected; index += 1) {
    pixels[index] = binary.charCodeAt(index);
  }
  return new ImageData(pixels, width, height);
}

/** "Lot Textures" 地表纹理 → 像素数据（compose v2 输入）。 */
async function loadSurfacePixels(): Promise<ImageData | null> {
  const url = props.lotSurfacePng;
  if (!url) return null;
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const element = new Image();
      element.onload = () => resolve(element);
      element.onerror = () => reject(new Error("lot surface failed"));
      element.src = url;
    });
    const canvas = document.createElement("canvas");
    canvas.width = image.width;
    canvas.height = image.height;
    const context = canvas.getContext("2d");
    if (!context) return null;
    context.drawImage(image, 0, 0);
    return context.getImageData(0, 0, image.width, image.height);
  } catch {
    return null;
  }
}

onMounted(async () => {
  await viewport.ready;
  await rebuildScene();
  await ensureGizmo();
});
onBeforeUnmount(() => {
  gizmo?.detach();
  gizmo?.dispose();
  gizmo = null;
});

/** 业务场景装配：模型材质 → 地面 → 六类 Unit → 路径折线。 */
async function assembleScene(ctx: Parameters<
  Parameters<typeof viewport.rebuild>[0]
>[0]) {
  const { viewer: instance, THREE } = ctx;
  specUniformRefs.length = 0;

  const payload = props.modelPayload;
  const modelObjects = payload ? await parseLotModelObjects(payload.glbs) : [];
  if (ctx.isStale()) {
    for (const object of modelObjects) disposeObject(object);
    return;
  }
  const whiteMaterial = new THREE.MeshStandardMaterial({
    color: 0xb8c2cc,
    roughness: 0.55,
    metalness: 0.12,
    side: THREE.DoubleSide,
  });
  // 逐 mesh 材质下标（与 glbs 同序）；材质分组的下标 → 该组 mesh 的材质列表
  const materialGroups: ThreeNamespace.MeshStandardMaterial[][] = (
    payload?.materials ?? []
  ).map(() => []);
  const tintResolved =
    props.renderMode === "refined" && payload
      ? await loadTintTextures(THREE, payload.materials ?? [], ctx.registerTextureUrl)
      : [];
  if (ctx.isStale()) return;
  // 5d 日/夜环境共享 uniform（全部 tint 材质引用同一组对象）
  const env = createSunEnv(THREE);
  envRefs = env;
  // 注：空腔质心锚定已被统计检验否定（lot_cavity_stats 400 样本，
  // d0-d1 配对 t=-5.15：bbox 中心到空腔质心反而更远）——建筑保持
  // 居中（bbox≈0 实证），mask 空腔与其错位另有机制（0x0CCB7FD2/D3
  // UV 字段为头号嫌疑，待取证）。
  for (const [index, object] of modelObjects.entries()) {
    const materialIndex = payload?.meshMaterialIndices[index] ?? 0;
    const uvKind = payload?.meshUvKinds[index] ?? 0;
    object.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (!mesh.isMesh) return;
      if (props.renderMode !== "refined") {
        mesh.material = whiteMaterial;
        return;
      }
      const tint = tintResolved[materialIndex];
      if (uvKind === 2 && tint?.tintTex && tint.paletteTex) {
        // facade tint 着色器：逐像素复刻 building4 链（tint 查表 → palette 查色）
        const [tinted, specUniform] = makeTintMaterial(THREE, tint, env);
        specUniformRefs.push(specUniform);
        mesh.material = tinted;
        return;
      }
      // GLB 的 COLOR_0（该 mesh 材质调色板的顶点色）→ GLTFLoader 的 color 属性
      const refined = new THREE.MeshStandardMaterial({
        vertexColors: Boolean(mesh.geometry.attributes.color),
        roughness: 0.82,
        metalness: 0,
        side: THREE.DoubleSide,
      });
      mesh.material = refined;
      if (uvKind === 1) materialGroups[materialIndex]?.push(refined);
    });
    instance.group("model").add(object);
  }
  if (props.renderMode === "refined" && payload) {
    applyDeferredMaterialMaps(
      THREE,
      payload,
      materialGroups,
      ctx.registerTextureUrl,
      ctx.isStale,
    );
  }

  // Lot 地面矩形（LotSize）；有 LotMask 时异步贴四色量化图。
  if (props.lotSize) {
    const ground = buildLotRect(THREE, props.lotSize);
    // 关闭自动更新：矩阵完全由 placement 逆决定，防止渲染循环覆盖。
    if (props.lotPlacement) {
      ground.matrix.copy(placementInverse(THREE, props.lotPlacement));
    }
    ground.matrixAutoUpdate = false;
    // 独立 lot 组：与建筑模型分开控制可见性（关模型不连地面一起隐藏，
    // 2026-09-13 用户对拍需求）。
    instance.group("lot").add(ground);
    if (props.lotMaskPng || props.lotAlbedoPng) {
      // v2：先加载地表纹理像素，失败/缺失时 compose 回退 v1
      const [surface, maskDims] = await Promise.all([
        loadSurfacePixels(),
        loadMaskImageDims(),
      ]);
      if (!surface) {
        // 精细渲染的材质替换依赖真实图集；静默回退占位 tile 会把沥青画成
        // 亮灰（2026-09-13 对拍教训），必须让用户看到原因。
        console.warn(
          "[lot-ground] 'Lot Textures' surface unavailable — refined ground will use placeholder tiles. Open SimCity_Graphics.package (and the lot's own package) for the real atlas.",
        );
      }
      const rawMask = maskDims
        ? loadRawMaskPixels(maskDims.width, maskDims.height)
        : null;
      applyGroundMask({
        rawMask,
        surface,
        THREE,
        ground,
        maskPng: props.lotMaskPng,
        albedoPng: props.lotAlbedoPng,
        lotSize: props.lotSize,
        refined: props.renderMode === "refined",
        lotColors: props.lotColors,
        lotColorsAuthored: props.lotColorsAuthored,
        isStale: ctx.isStale,
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
    ctx.unitObjects.set(unitId(unit), object);
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

  if (!keyLightInitialized) {
    instance.setKeyLight(45, 55);
    keyLightInitialized = true;
  }
  applySun();
  applyBrightness();
  viewport.applyGroupVisibility(props.groupVisibility);
  viewport.applyUnitVisibility(props.hiddenUnits);
  viewport.applySelection(props.selectedId);
}

/** 日/夜亮度：仅缩放环境四灯；地块真实光源保持常亮（夜间灯依然亮）。 */
function applyBrightness() {
  // 5d：夜间环境光随白昼因子压暗（亮度滑杆仍是用户侧总倍率）
  viewport.viewer.value?.setEnvironmentBrightness(
    brightness.value * (0.22 + 0.78 * dayFactor(timeOfDay())),
  );
}

defineExpose({ captureRender: viewport.captureRender });

watch(
  () => [props.modelPayload, props.renderMode, props.grouping],
  () => void rebuildScene(),
);
watch(
  () => props.groupVisibility,
  () => viewport.applyGroupVisibility(props.groupVisibility),
  { deep: true },
);
watch(
  () => props.hiddenUnits,
  () => viewport.applyUnitVisibility(props.hiddenUnits),
);
watch(
  () => props.selectedId,
  () => viewport.applySelection(props.selectedId),
);
// 工具/选中/rebuild 代数变化 → 重挂或摘除手柄（选中对象会被重建）
watch(
  [() => props.tool, () => props.selectedId, viewport.revision],
  () => updateGizmo(),
);
</script>

<template>
  <div class="viewport-pane">
    <div :ref="(el) => (viewport.container.value = el as HTMLElement | null)" class="viewport-3d" />
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
        :aria-label="$t('package.lodSwitch')"
        :title="$t('package.lodSwitch')"
        :aria-pressed="lodPanelOpen"
        :class="{ active: lodPanelOpen }"
        @click="lodPanelOpen = !lodPanelOpen"
      >
        <FIcon name="Layers" :size="13" aria-label="" />
      </button>
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
        :aria-label="$t('package.modelInfo')"
        :title="$t('package.modelInfo')"
        :aria-pressed="infoPanelOpen"
        :class="{ active: infoPanelOpen }"
        @click="infoPanelOpen = !infoPanelOpen"
      >
        <FIcon name="Info" :size="13" aria-label="" />
      </button>
      <button
        type="button"
        :aria-label="$t('package.resetView')"
        :title="$t('package.resetView')"
        @click="viewport.resetView()"
      >
        <FIcon name="RotateCcw" :size="13" aria-label="" />
      </button>
    </div>
    <div
      v-if="lodPanelOpen"
      class="viewport-overlay lod-mask"
      @click.self="lodPanelOpen = false"
    >
      <div class="lod-card" role="dialog" :aria-label="$t('package.lodSwitch')">
        <header class="lod-card-header">
          <span>{{ $t("package.lodSwitch") }}</span>
          <button
            type="button"
            class="lod-close"
            :aria-label="$t('common.close')"
            @click="lodPanelOpen = false"
          >
            <FIcon name="X" :size="13" aria-label="" />
          </button>
        </header>
        <div class="lod-tiles">
          <button
            v-for="(lod, index) in modelLods"
            :key="index"
            type="button"
            class="lod-tile"
            :class="{
              active: index === activeLod,
              missing: lod === null,
            }"
            :disabled="lod === null"
            @click="emit('switch-lod', index); lodPanelOpen = false"
          >
            <span class="lod-tile-level">LOD{{ index + 1 }}</span>
            <span v-if="lod === null" class="lod-tile-missing">{{
              $t("package.lodMissing")
            }}</span>
          </button>
        </div>
      </div>
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
    <div v-if="infoPanelOpen" class="viewport-overlay info-panel">
      <div class="info-card" role="dialog" :aria-label="$t('package.modelInfo')">
        <header class="lod-card-header">
          <span>{{ $t("package.modelInfo") }}</span>
          <div class="info-header-actions">
            <button
              type="button"
              class="lod-close"
              :title="$t('package.copyDiagnostics')"
              :aria-label="$t('package.copyDiagnostics')"
              :disabled="!modelPayload?.diagnostics"
              @click="copyDiagnostics"
            >
              <FIcon :name="infoCopied ? 'Check' : 'Copy'" :size="13" aria-label="" />
            </button>
            <button
              type="button"
              class="lod-close"
              :aria-label="$t('common.close')"
              @click="infoPanelOpen = false"
            >
              <FIcon name="X" :size="13" aria-label="" />
            </button>
          </div>
        </header>
        <pre class="info-pre">{{ modelPayload?.diagnostics || $t("package.modelInfoEmpty") }}</pre>
      </div>
    </div>
    <div
      class="viewport-overlay viewport-toolrail"
      role="group"
      :aria-label="$t('package.editorTools')"
    >
      <button
        v-for="entry in [
          { tool: 'select', icon: 'MousePointer', label: 'package.toolSelect' },
          { tool: 'translate', icon: 'Move', label: 'package.toolMove' },
          { tool: 'rotate', icon: 'RotateCw', label: 'package.toolRotate' },
          { tool: 'scale', icon: 'Expand', label: 'package.toolScale' },
        ]"
        :key="entry.tool"
        type="button"
        :class="{ active: (tool ?? 'select') === entry.tool }"
        :aria-pressed="(tool ?? 'select') === entry.tool"
        :title="`${$t(entry.label)} (${entry.tool === 'translate' ? 'G' : entry.tool === 'rotate' ? 'R' : entry.tool === 'scale' ? 'S' : 'Esc'})`"
        @click="emit('select-tool', entry.tool as EditorTool)"
      >
        <FIcon :name="entry.icon" :size="14" aria-label="" />
      </button>
    </div>
    <div class="viewport-overlay viewport-visibility" role="group" :aria-label="$t('package.visibilityToggles')">
      <button
        v-for="name in ['model', 'lot', 'lights', 'props', 'decals', 'effects', 'spawners', 'paths']"
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
.lod-mask {
  background: color-mix(in srgb, var(--surface) 55%, transparent);
  display: grid;
  inset: 0;
  place-items: center;
  z-index: 2;
}
.lod-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg, 0 8px 28px rgb(0 0 0 / 0.35));
  display: grid;
  gap: 12px;
  padding: 14px;
  width: min(340px, calc(100% - 32px));
}
.lod-card-header {
  align-items: center;
  color: var(--foreground);
  display: flex;
  font-size: 13px;
  font-weight: 650;
  justify-content: space-between;
}
.lod-close {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  min-height: 24px;
  min-width: 24px;
  justify-content: center;
}
.lod-close:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.lod-tiles {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(4, 1fr);
}
.lod-tile {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-md, var(--radius-sm));
  color: var(--foreground);
  cursor: pointer;
  display: grid;
  font: inherit;
  gap: 2px;
  justify-items: center;
  min-height: 52px;
  padding: 8px 4px;
  transition: border-color 120ms ease, box-shadow 120ms ease;
}
.lod-tile:hover:not(:disabled) {
  border-color: var(--accent);
}
.lod-tile.active {
  border-color: var(--accent);
  box-shadow:
    0 0 0 1px var(--accent),
    0 0 10px color-mix(in srgb, var(--accent) 45%, transparent);
}
.lod-tile:disabled {
  color: var(--subtle-foreground);
  cursor: not-allowed;
  opacity: 0.55;
}
.lod-tile-level {
  font-size: 12.5px;
  font-weight: 650;
}
.lod-tile-missing {
  color: var(--subtle-foreground);
  font-size: 10.5px;
}
.info-panel {
  right: 10px;
  top: 48px;
}
.info-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg, 0 8px 28px rgb(0 0 0 / 0.35));
  display: grid;
  gap: 8px;
  max-height: min(60vh, 520px);
  padding: 12px;
  width: min(540px, calc(100vw - 32px));
}
.info-header-actions {
  display: inline-flex;
  gap: 2px;
}
.info-pre {
  color: var(--foreground);
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
  overflow: auto;
  user-select: text;
  white-space: pre;
}
.viewport-visibility {
  display: grid;
  gap: 3px;
  left: 10px;
  top: 10px;
}
.viewport-toolrail {
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  gap: 2px;
  left: 10px;
  padding: 3px;
  top: 50%;
  transform: translateY(-50%);
}
.viewport-toolrail button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 28px;
  min-width: 28px;
}
.viewport-toolrail button:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.viewport-toolrail button.active {
  background: var(--accent);
  color: var(--foreground);
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
