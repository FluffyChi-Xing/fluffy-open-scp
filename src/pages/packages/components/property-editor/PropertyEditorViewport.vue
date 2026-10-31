<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { disposeObject } from "@/lib/three-viewer";
import { getLotModelObjects } from "@/lib/three-gltf";
import { renderTelemetry } from "@/lib/renderTelemetry";
import type { RenderTelemetryTrigger } from "@/lib/renderTelemetry";
import type * as ThreeNamespace from "three";
import type {
  DecalUnit,
  DecalUnitTexture,
  EffectDefinitionData,
  LotModelLodRef,
  LotModelPayload,
  LotUnitDto,
} from "@/api/tauri";
import type { ModelState, UnitGrouping } from "./usePropertyEditorSession";
import { unitLabel } from "./usePropertyEditorSession";
import { unitId, unitMatrix } from "./unitGizmos";
import {
  buildPathLine,
  buildRealLightUnit,
  buildUnitObject,
} from "@/render/exec/three/overlays";
import { kindGroup, stageDecals, stageUnits } from "@/render/session/stages";
import { stageGround } from "@/render/session/groundStage";
import { loadSimParts, type SimPart } from "./simAssets";
import { renderRefinedAssetUnit, type UnitRenderContext } from "./renderers";
import type {
  ImageDecodeRequest,
  ImageDecodeResponse,
  Pixels,
} from "./imageDecode";
import { useEditorViewport, VIEWPORT_GROUPS } from "./useEditorViewport";
import type { CameraPreset } from "@/lib/three-viewer";
import {
  applySunEnv,
  createSunEnv,
  dayFactor,
  type SunEnvRefs,
} from "./refinedRender";
import { threeToRowMajor } from "./unitEditLayer";
import { installHejlToneMapping } from "@/lib/hejlTonemapping";
import {
  createParticleEffect,
  type ParticleEffectView,
} from "../property-editor-next/effects";
import type { TransformControls } from "three/examples/jsm/controls/TransformControls.js";
import {
  lotDimensionPlan,
  type DimensionAnchor,
} from "@/render/plan/ground";
import { createLotDimensions } from "@/render/exec/three/ground";
import { executeModel } from "@/render/exec/three/model";
import { buildDecalBody } from "@/render/exec/three/decalBody";

export type EditorTool = "select" | "translate" | "rotate" | "scale";

/**
 * Lot 场景装配层（业务形态相关）：在 useEditorViewport 底座上组合
 * refinedRender（tint shader/日夜环境）、editorGround（地面）与
 * unitGizmos（六类 Unit）完成 SCP lot 视口。浮层 UI 亦在此层。
 * 底座/业务拆分见 docs/roadmap/asset-development.md §二（PE-重构-1）。
 */
const props = defineProps<{
  groundOnly?: boolean;
  modelPayload: LotModelPayload | null;
  /** LOD1~LOD4 资源位置（index 0 = LOD1）；缺失级为 null。 */
  modelLods: (LotModelLodRef | null)[];
  activeLod: number;
  renderMode: "default" | "refined";
  grouping: UnitGrouping;
  lotSize: [number, number] | null;
  lotTilePeriod: [number, number] | null;
  /** LotPlacementTransform 行主序 12 floats；地面矩形取其逆对齐建筑。 */
  lotPlacement: number[] | null;
  /** LotColor1-4 RGBA（A = 图案/法线图集格号 0-15）。 */
  lotColors: [number, number, number, number][];
  /** LotColor1-4 是否实际存在（false = 引擎回退调色板，平色参与渲染）。 */
  lotColorsAuthored: boolean[];
  /** LotBorderColor1-4 的 sRGB RGB（mask 渐变带描边平色）。 */
  lotBorderColors: [number, number, number][];
  /** borderWidth1-4（边框带半宽）；全 0 = 无边框。 */
  lotBorderWidths: number[];
  lotColorHeights?: number[];
  lotBorderHeights?: number[];
  /** 边框带图案索引（LotBorderColor.A，0-15）。 */
  lotBorderPatternIndices: number[];
  /** 底图格索引（后端三级来源：0x0CCB7FD6 → 推导 → 8）。 */
  lotBaseTile: number;
  /** Authored lot-space overlay centre; null uses the declared unit bbox centre. */
  lotOverlayBoxOffset: [number, number] | null;
  /** Declared unit bounding-box centre (0x00F9EFBA), used only if offset is absent. */
  lotModelBboxCenter: [number, number] | null;
  /** P2 精细替换：prop resourceID → 已解析 LOTM 载荷（会话旁路加载）。 */
  propModels: Map<number, LotModelPayload>;
  /** 树 prop 资源 id 集合（source=tree/tree_model）→ 树专用渲染分支。 */
  propTreeIds: Set<number>;
  /** 模型树路线（§1.6）：impostor 源 3D 模型 LOTM 载荷（≤4 形状，共享）；
   * 空数组 = 后端模型缺席 → 公告板回落。 */
  treeModelPayloads: LotModelPayload[];
  /** 树公告板图集 PNG（base64，2×2 四树格；3D 模型加载失败的回落通道）。 */
  treeAtlasPng: string | null;

  lotMaskPng: string | null;
  /** LotMask 原始通道权重图（v4 软混合输入）。 */
  /** LotMask 原始通道权重（未压缩 RGBA base64；A = LC4 权重）。 */
  lotMaskRawRgba: string | null;
  /** 默认模式地表反照率（通道平色+底图格；缺失时回退量化图）。 */
  lotAlbedoPng: string | null;
  /** 精细模式贴花纹理（按 decal 的 category+index 对应）。 */
  decalTextures: DecalUnitTexture[];
  /** "Lot Textures" 地表共享纹理（data URL；精细模式地面 v2 用）。 */
  lotSurfacePng: string | null;
  /** 全局共享法线图集（s15）data URL：图案质感 normalMap。 */
  lotNormalAtlasPng: string | null;
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
  /** 表面浮雕开关（默认 false）：开启近景 cone-step relief。 */
  reliefEnabled?: boolean;
  /** 动态招牌开关（默认 false = 静态量化合成招牌）：开启后 SDF 族进入
   * dev LED 扫掠动画并启动 rAF 时钟。顶部工具条复选框驱动（2026-10-05
   * 十轮：原视口 ⚡ 按钮用户找不到，挪入顶部工具条"供电"旁）。 */
  neonAnim?: boolean;
  renderEffects?: boolean;
  effectDefinitions?: Map<number, EffectDefinitionData>;
  /** 破洞贴花假内景光参数 [光强因子, 半径因子]（0x0DA76A05/06）；缺失 = 无。 */
  decalLight?: [number, number] | null;
  /** 当前编辑工具（select = 仅拾取；其余挂 TransformControls 手柄）。 */
  tool?: EditorTool;
  /** 编辑模式（解锁）：工具栏可见、画布可拖放、name-tag 带删除钮。 */
  editEnabled?: boolean;
  /** 拖入的直挂模型载荷（instance → LOTM 载荷；真模型渲染用）。 */
  addedModelPayloads?: Map<string, LotModelPayload>;
  /** 放置后待选中：分组重建且新单元入组时自动选中（shell 传入）。 */
  pendingSelectId?: string | null;
  /**
   * 挂起（PE sheet 关闭中）：全部重建触发器静默——关闭时 shell 会复位
   * renderMode，不挂起就会在卸载 EcoGame 包的同时触发一次完整重建
   * （关闭卡顿根因，2026-10-07）。恢复可见时由 shell 翻回 false，
   * 视口补一次重建对齐当前渲染模式。
   */
  suspended?: boolean;
  /**
   * 模型来源（mod 工作台外部导入）：native = 游戏 LOTM（白模/游戏 tint
   * 材质管线）；glb = 外部导入 GLB（Blender 六槽烘焙贴图已内嵌在 glTF
   * 材质中，保留原材质，不做白模替换与 tint/deferred 贴图链）。
   */
  modelSource?: "native" | "glb";
  /**
   * 草稿伪单元：导入的建筑模型本身可被选中并用手柄做四种变换。
   * 选中 id 固定为 "model"；变换提交走 commit-transform("model", matrix)。
   */
  modelSelectable?: boolean;
  /** 模型伪单元的变换（行主序 12 floats；rebuild 时应用到包装组）。 */
  modelTransform?: number[] | null;
  /** 草稿模型放置原点平移（水平居中 + 底面坐 y=0；世界空间，装配时
   * 应用到各模型根——地面恒在原点平面）。 */
  modelOrigin?: [number, number, number] | null;
  /** Camera workbench values mapped onto the real Three viewer camera. */
  cameraPreset?: CameraPreset | null;
  /** Camera workbench depth-of-field (game meters / aperture / softness). */
  cameraFocus?: { distance: number; aperture: number; softness: number } | null;
}>();
const emit = defineEmits<{
  select: [id: string | null];
  "toggle-layer": [name: string];
  "switch-lod": [index: number];
  "select-tool": [tool: EditorTool];
  /** 手柄拖拽结束提交变换（行主序 12 floats），由壳落本地编辑命令。 */
  "commit-transform": [id: string, matrix: number[]];

  /** 删除组件（name-tag 垃圾桶按钮）。 */
  "delete-unit": [id: string];
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
const { t } = useI18n();
/** 悬停中的 Unit（dashed 描边 + 左上角小字标签）。 */
const hoveredId = ref<string | null>(null);
/** 小人部件资产（全局，进程级缓存）：spawner 真小人渲染。 */
const simParts = ref<SimPart[]>([]);
onMounted(() => {
  if (props.groundOnly) return;
  void loadSimParts().then((parts) => {
    simParts.value = parts;
  });
});
const viewport = useEditorViewport({
  maxPixelRatio: 2,
  interactionPixelRatio: 1,
  onTapUnit: (id) => emit("select", id),
  onHoverUnit: (id) => {
    hoveredId.value = id;
  },
});
function applyCameraPreset() {
  const preset = props.cameraPreset;
  if (preset && viewport.viewer.value) viewport.viewer.value.setCameraPreset(preset);
}
watch(
  [() => props.cameraPreset, viewport.viewer, viewport.revision],
  applyCameraPreset,
  { deep: true, immediate: true },
);
// 摄像机工作台景深(仅 cameraControlled 链路传入;其他场景恒 null 不启用)。
watch(
  [() => props.cameraFocus, viewport.viewer],
  ([focus]) => {
    viewport.viewer.value?.setDepthOfField(focus ?? null);
  },
  { deep: true, immediate: true },
);
// 草稿模型的拾取开关：同步到射线目标组（模型关闭后取消其选中）
watch(
  () => props.modelSelectable === true,
  (pickable) => {
    viewport.setModelPickable(pickable);
    if (!pickable && props.selectedId === "model") emit("select", null);
  },
  { immediate: true },
);

// ---------------------------------------------------------------------------
// 拾取反馈（2026-10-06 二轮）：轮廓壳（viewer 反转壳体，hover 虚线/选中
// 实线，贴模型剪影）+ 锚定组件原点的文字标签。屏幕空间 AABB 方框已撤
// （精细灯光 bbox 爆炸 + 方框随视角变形，用户反馈）。
// 地面/建筑模型不属于 Unit，不参与。
// ---------------------------------------------------------------------------
const hoverTagEl = ref<HTMLElement | null>(null);
const selectTagEl = ref<HTMLElement | null>(null);

// ---- Lot 三维尺度标注（长/宽/高，自地块一角）----
// 线段在 viewer "dimensions" 组（editorGround.buildLotDimensions）；
// 文字标签随帧投影（同 pick-tag 通道）。game = lot 局部锚点经地面矩阵
// 换算后的游戏系坐标（world.localToWorld 后投影）。
const dimLabels = ref<
  (DimensionAnchor & { game: [number, number, number] })[]
>([]);
const dimLabelEls = ref<(HTMLElement | null)[]>([]);
const dimAxisLabel = computed(() => ({
  length: t("package.dimLength"),
  width: t("package.dimWidth"),
  height: t("package.dimHeight"),
}));
const unitById = computed<Map<string, LotUnitDto>>(() => {
  const map = new Map<string, LotUnitDto>();
  const groups = props.grouping;
  for (const unit of [
    ...groups.lights,
    ...groups.decals,
    ...groups.props,
    ...groups.effects,
    ...groups.spawners,
    ...groups.pathPoints,
  ]) {
    map.set(unitId(unit), unit);
  }
  return map;
});
function overlayLabel(id: string): string {
  // 草稿模型伪单元（整栋建筑）：显示语义名而非内部 id
  if (id === "model") return t("package.modelUnitLabel");
  const unit = unitById.value.get(id);
  return unit ? unitLabel(unit, t) : id;
}

/** 组件原点 → 容器相对屏幕坐标（标签锚点）。 */
function tagAnchor(id: string): { x: number; y: number } | null {
  const instance = viewport.viewer.value;
  if (!instance) return null;
  const object = viewport.unitObjects.get(id);
  if (!object || !object.visible) return null;
  const world = new instance.THREE.Vector3();
  object.getWorldPosition(world);
  const screen = instance.worldToScreen(world);
  if (screen.behind) return null;
  return { x: screen.x, y: screen.y };
}

function placeTag(el: HTMLElement | null, id: string | null, label: string) {
  if (!el) return;
  if (!id) {
    el.style.display = "none";
    return;
  }
  const anchor = tagAnchor(id);
  if (!anchor) {
    el.style.display = "none";
    return;
  }
  el.style.display = "block";
  el.style.left = `${anchor.x}px`;
  el.style.top = `${anchor.y}px`;
  // 写入文本子节点而非容器 textContent——容器内还有删除按钮节点，
  // 整体赋值会把它抹掉（真机勘误 2026-10-06）。
  const textEl = el.querySelector(".pick-tag-text") as HTMLElement | null;
  if (textEl) textEl.textContent = label;
}

/** 尺度标签逐帧投影：游戏系锚点 → world → 屏幕像素（居中锚定）。 */
function updateDimLabels() {
  const instance = viewport.viewer.value;
  if (!instance) return;
  // 跟随 dimensions 组可见性开关（线段隐藏时标签同隐）
  const hidden = props.groupVisibility.dimensions === false;
  for (let index = 0; index < dimLabels.value.length; index += 1) {
    const el = dimLabelEls.value[index];
    const dim = dimLabels.value[index];
    if (!el || !dim) continue;
    if (hidden) {
      el.style.display = "none";
      continue;
    }
    const scenePoint = instance.world.localToWorld(
      new instance.THREE.Vector3(...dim.game),
    );
    const screen = instance.worldToScreen(scenePoint);
    if (screen.behind) {
      el.style.display = "none";
      continue;
    }
    el.style.display = "block";
    el.style.left = `${screen.x}px`;
    el.style.top = `${screen.y}px`;
  }
}

function updatePickOverlay() {
  const hover = hoveredId.value;
  const selected = props.selectedId;
  placeTag(
    hoverTagEl.value,
    hover && hover !== selected ? hover : null,
    hover ? overlayLabel(hover) : "",
  );
  placeTag(selectTagEl.value, selected, selected ? overlayLabel(selected) : "");
  updateDimLabels();
}

// ---- 编辑模式：组件库拖放放置 + 删除 ----
// 拖放监听挂 window（捕获级）：组件库 FSheet 的全屏遮罩（pointer-events
// auto、z 80）会拦截画布上的 drop——面板开着拖放时 drop 的目标元素是
// 遮罩而非画布，元素级处理器永远收不到（真机勘误 2026-10-06）。

function onDeleteSelected() {
  if (props.selectedId) emit("delete-unit", props.selectedId);
}

/** 轮廓壳应用：hover 虚线；选中实线经 viewer.setSelected 内部应用。 */
function applyOutlines() {
  const instance = viewport.viewer.value;
  if (!instance) return;
  const hover = hoveredId.value;
  const hoverObject =
    hover && hover !== props.selectedId
      ? (viewport.unitObjects.get(hover) ?? null)
      : null;
  instance.setHoverOutline(hoverObject);
  const selected = props.selectedId;
  instance.setSelected(
    selected ? (viewport.unitObjects.get(selected) ?? null) : null,
  );
}
watch(viewport.viewer, (instance) => {
  instance?.setOnFrame(() => updatePickOverlay());
  applyOutlines();
  updatePickOverlay();
});
watch([hoveredId, () => props.selectedId, viewport.revision], () => {
  applyOutlines();
  updatePickOverlay();
});
// 放置后待选中：分组重建（revision 前进）且新单元已入组时选中
watch([() => props.pendingSelectId, viewport.revision], () => {
  const pending = props.pendingSelectId;
  if (pending) emit("select", pending);
});
onBeforeUnmount(() => {
  viewport.viewer.value?.setOnFrame(null);
  const instance = viewport.viewer.value;
  instance?.setHoverOutline(null);
  instance?.setSelectedOutline(null);
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
// 精细渲染为 HDR 管线（interiorMap.a×16 自发光、×256 艺术自发光）：
// 接入游戏 post 管线的 hejlToneMap 逐字公式（P3，hejlTonemapping.ts）——
// 替换此前 ACES@1.12 折中；曝光 1.12 延续旧校准（引擎 sunSky.mSunColor.w
// 未知，可调）
watch(
  viewport.viewer,
  (instance) => {
    if (!instance) return;
    installHejlToneMapping(instance.THREE);
    instance.setToneMapping(instance.THREE.CustomToneMapping, 1.12);
  },
  { immediate: true },
);
watch(brightness, () => applyBrightness());

/** 存活 tint 材质的 uSpecMode uniform 引用（通道实验热切换，免重建）。 */
const specUniformRefs: { value: number }[] = [];

let envRefs: SunEnvRefs | null = null;

// ---------------------------------------------------------------------------
// 会话级缓存：编辑操作（transform/undo → grouping 全量重建）的热路径上，
// 解码与合成结果完全不变，按「输入源字符串身份」缓存跨 rebuild 复用。
// ---------------------------------------------------------------------------

/** 图像解码缓存（key = 源 data URL / base64 字符串身份；容量上限防跨会话累积）。 */
const imageDataCache = new Map<string, Promise<ImageData | null>>();
const IMAGE_DATA_CACHE_CAP = 16;
/** 尺寸探测缓存（mask 图宽高，同键空间）。 */
const imageDimsCache = new Map<
  string,
  Promise<{ width: number; height: number } | null>
>();

function cacheGetOrLoad<V>(
  cache: Map<string, Promise<V>>,
  key: string,
  load: () => Promise<V>,
): Promise<V> {
  const hit = cache.get(key);
  if (hit) return hit;
  const pending = load();
  cache.set(key, pending);
  if (cache.size > IMAGE_DATA_CACHE_CAP) {
    const oldest = cache.keys().next().value as string | undefined;
    if (oldest !== undefined && oldest !== key) cache.delete(oldest);
  }
  return pending;
}

function decodeImageData(url: string): Promise<ImageData | null> {
  return new Promise((resolve) => {
    const element = new Image();
    element.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = element.width;
      canvas.height = element.height;
      const context = canvas.getContext("2d");
      if (!context) return resolve(null);
      context.drawImage(element, 0, 0);
      resolve(context.getImageData(0, 0, element.width, element.height));
    };
    element.onerror = () => resolve(null);
    element.src = url;
  });
}

// ---- 图像像素解码 Worker（2026-10-08 OffscreenCanvas 轮）----
// 地表共享图集（2048² 级）的 drawImage + getImageData 是首载主线程的
// 一块大额同步占用；移入 Worker（createImageBitmap + OffscreenCanvas
// 全程离线执行），主线程只收 transferable 像素。失败回退主线程同款
// 实现（下方 decodeImageData），调用方无感。
let imageDecodeWorker: Worker | null = null;
let imageDecodeWorkerBroken = false;
let nextImageDecodeJobId = 0;
const pendingImageDecodeJobs = new Map<
  number,
  { resolve: (pixels: Pixels) => void; reject: (error: Error) => void }
>();

function getImageDecodeWorker(): Worker | null {
  if (imageDecodeWorker) return imageDecodeWorker;
  if (imageDecodeWorkerBroken) return null;
  try {
    imageDecodeWorker = new Worker(
      new URL("./imageDecodeWorker.ts", import.meta.url),
      { type: "module" },
    );
    imageDecodeWorker.onmessage = (
      event: MessageEvent<ImageDecodeResponse>,
    ) => {
      const job = pendingImageDecodeJobs.get(event.data.id);
      if (!job) return;
      pendingImageDecodeJobs.delete(event.data.id);
      if (event.data.error || !event.data.pixels) {
        imageDecodeWorkerBroken = true;
        job.reject(new Error(event.data.error ?? "image decode empty"));
        return;
      }
      job.resolve(event.data.pixels);
    };
    imageDecodeWorker.onerror = () => {
      imageDecodeWorkerBroken = true;
      for (const job of pendingImageDecodeJobs.values()) {
        job.reject(new Error("image decode worker crashed"));
      }
      pendingImageDecodeJobs.clear();
    };
    return imageDecodeWorker;
  } catch {
    imageDecodeWorkerBroken = true;
    return null;
  }
}

/** Worker 解码（PNG/dataURL → Pixels）；不可用时回退主线程 decodeImageData。 */
async function decodePixelsForUrl(url: string): Promise<ImageData | null> {
  const worker = getImageDecodeWorker();
  if (!worker) return decodeImageData(url);
  const id = ++nextImageDecodeJobId;
  const pixels = await new Promise<Pixels>((resolve, reject) => {
    pendingImageDecodeJobs.set(id, { resolve, reject });
    worker.postMessage({ id, url } satisfies ImageDecodeRequest);
  }).catch(() => null);
  if (!pixels) return decodeImageData(url);
  return new ImageData(pixels.data, pixels.width, pixels.height);
}

/** 贴花解码纹理缓存（key = DecalUnitTexture 对象身份；会话更换即失效回收）。 */
let decalTextureCacheOwner: DecalUnitTexture[] | null = null;
const decalTextureCache = new Map<DecalUnitTexture, ThreeNamespace.Texture>();

function releaseDecalTextureCache(): void {
  for (const texture of decalTextureCache.values()) texture.dispose();
  decalTextureCache.clear();
  decalTextureCacheOwner = null;
}

async function getDecalTexture(
  THREE: typeof ThreeNamespace,
  texture: DecalUnitTexture,
): Promise<ThreeNamespace.Texture | null> {
  if (!texture.png) return null;
  if (decalTextureCacheOwner !== props.decalTextures) {
    releaseDecalTextureCache();
    decalTextureCacheOwner = props.decalTextures;
  }
  const hit = decalTextureCache.get(texture);
  if (hit) return hit;
  try {
    // png 是**裸 base64 字符串**（四色解码 PNG），必须走 data URL——
    // 不能 new Blob([png])：那会把 base64 文本当字节，解码必然失败
    //（2026-09-26 回归：全部贴花回退绿色占位 gizmo 的根因）。
    const decoded = await new THREE.TextureLoader()
      .loadAsync(`data:image/png;base64,${texture.png}`)
      .catch((error: unknown) => {
        console.warn(
          "[decal] 纹理 data URL 解码失败",
          texture.idInstance,
          error,
        );
        return null;
      });
    if (!decoded) return null;
    // sign/graffiti raw 掩码纹理 = 数据通道（阈值 0.5 作用于线性值），
    // 必须线性采样；破洞内景图 = 颜色纹理保持 sRGB。注意破洞条目**也带
    // colors 字段**（探针 hole_decal_dump 实证），判据必须按材质实例分流，
    // 不能只看 colors 有无。
    const isHoleTexture = DECAL_HOLE_MATERIALS.has(
      (texture.materialInstance ?? 0) >>> 0,
    );
    if (texture.colors && !isHoleTexture) {
      decoded.colorSpace = THREE.NoColorSpace;
    } else {
      decoded.colorSpace = THREE.SRGBColorSpace;
    }
    // 采样器口径（引擎变体对象：sign 族 base pass 之后全是 LINEAR×3 + mip；
    // graffiti 同 LINEAR）：
    // - 招牌/涂鸦 raw/四色解码源（76708d5 后源图已正确）：Linear + mip =
    //   游戏观感"锐利而边缘平滑"——POINT 的锯齿是 09-30 误治（模糊的病根
    //   是纹理源错误，已修）。
    // - 四色量化回退（quantized，32px 调色板色）：POINT 保像素画可读。
    // - 破洞：Linear + 禁 mip（视差采样口径）。
    const quantized = texture.quantized === true;
    if (quantized) {
      decoded.magFilter = THREE.NearestFilter;
      decoded.minFilter = THREE.NearestFilter;
      decoded.generateMipmaps = false;
    } else {
      decoded.generateMipmaps = true;
      decoded.minFilter = THREE.LinearMipmapLinearFilter;
    }
    // 破洞 flipY=false 特判已随体积盒路径退役（2026-10-05 二轮）：投影
    // 路径与其他族共用 UV 口径（文字/图案方向对拍确认），不再单独翻转。
    decoded.anisotropy = viewport.viewer.value?.maxAnisotropy ?? 1;
    decalTextureCache.set(texture, decoded);
    return decoded;
  } catch {
    return null;
  }
}

/**
 * 上一次装配/增量应用的 unit DTO 快照（id → unit）：增量更新判定的对照物。
 * 全量 rebuild 收尾与增量应用成功后都会刷新。
 */
let lastUnitsSnapshot = new Map<string, LotUnitDto>();

function buildUnitsSnapshot(grouping: UnitGrouping): Map<string, LotUnitDto> {
  const map = new Map<string, LotUnitDto>();
  for (const unit of [
    ...grouping.lights,
    ...grouping.decals,
    ...grouping.props,
    ...grouping.effects,
    ...grouping.spawners,
    ...grouping.pathPoints,
  ]) {
    map.set(unitId(unit), unit);
  }
  return map;
}

/** 除 transform 外的 DTO 等价判定（非 transform 字段变化必须走全量重建）。 */
function unitEqualsIgnoringTransform(a: LotUnitDto, b: LotUnitDto): boolean {
  if (a.kind !== b.kind) return false;
  const restA: Record<string, unknown> = { ...a };
  const restB: Record<string, unknown> = { ...b };
  delete restA.transform;
  delete restB.transform;
  return JSON.stringify(restA) === JSON.stringify(restB);
}

/** DTO 的缩放倍率字段（仅 prop/decal 携带；其余 kind 无此字段视为 1）。 */
function unitScaleValue(unit: LotUnitDto): number | null {
  return unit.kind === "prop" || unit.kind === "decal" ? unit.scale : null;
}

/**
 * 仅 DTO scale（缩放倍率）变化的判定：返回新值/旧值倍率（旧值 null 视
 * 为 1）；其余字段有任何差异返回 null（须全量重建）。
 */
function scaleOnlyDelta(a: LotUnitDto, b: LotUnitDto): number | null {
  if (a.kind !== b.kind) return null;
  const restA: Record<string, unknown> = { ...a, scale: 0 };
  const restB: Record<string, unknown> = { ...b, scale: 0 };
  delete restA.transform;
  delete restB.transform;
  if (JSON.stringify(restA) !== JSON.stringify(restB)) return null;
  const previous = unitScaleValue(a) ?? 1;
  const next = unitScaleValue(b) ?? 1;
  if (previous === next || previous <= 1e-8) return null;
  return next / previous;
}

const timeOfDay = () => props.timeOfDay ?? 12;

function applySun() {
  if (envRefs) {
    // 共享 uniform 热切换（不重建材质）——按需渲染下必须显式请求重绘。
    applySunEnv(envRefs, timeOfDay(), props.powered);
    // 精细模式：key 光挂 env 太阳（方向/色），地面 Lambert 由它着色并接收
    // 建筑投影；默认模式保持白模滑杆口径。
    if (props.renderMode === "refined") {
      viewport.viewer.value?.setSunFromEnv(
        envRefs.sunDir.value,
        envRefs.sunColor.value,
      );
    }
    viewport.viewer.value?.invalidate();
  }
}
watch([() => props.specExperiment, () => props.specMode], () => {
  for (const uniform of specUniformRefs) uniform.value = 2;
  viewport.viewer.value?.invalidate();
});

/**
 * 浮雕开关热切换：只改每材质的 uReliefEnabled uniform，不重建场景/
 * 材质/贴图。平面建筑（reliefAvailable=false）保持关闭。
 */
function applyReliefSwitch(enabled: boolean) {
  const viewer = viewport.viewer.value;
  if (!viewer) return;
  let changed = false;
  viewer.group("model").traverse((child) => {
    const mesh = child as ThreeNamespace.Mesh;
    if (!mesh.isMesh) return;
    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    for (const material of materials) {
      const uniform = material?.userData?.uReliefEnabled as
        | { value: number }
        | undefined;
      if (!uniform) continue;
      const available = material.userData.reliefAvailable !== false;
      uniform.value = enabled && available ? 1 : 0;
      changed = true;
    }
  });
  if (changed) viewer.invalidate();
}
watch(() => props.reliefEnabled, (enabled) => applyReliefSwitch(enabled === true));
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


/** TransformControls 手柄（PE-重构-3）：拖拽期间挂起轨道相机，
 * 抬手提交一次行主序矩阵命令。helper 加入 scene 根（相机空间朝向）。 */
let gizmo: TransformControls | null = null;
let dragStartMatrix: ThreeNamespace.Matrix4 | null = null;
/** 默认主灯方位只初始化一次（用户调节后 rebuild 不再重置）。 */
let keyLightInitialized = false;
/** 上一次 rebuild 的模型载荷（身份变化 = 模型/LOD 更换 → 重构图）。 */
let lastPayload: LotModelPayload | null | undefined;

/** 对象 TRS → 单元变换语义 TRS：除掉渲染器烘焙的 DTO scale
 * （userData.incrementalScale）——提交的 override 矩阵在全量重建会被
 * 渲染器再乘回来，不除 = 编辑过的树在下次重建二次缩放（0.06²）。 */
function unitSemanticTRS(object: ThreeNamespace.Object3D): {
  position: ThreeNamespace.Vector3;
  quaternion: ThreeNamespace.Quaternion;
  scale: ThreeNamespace.Vector3;
} {
  const THREE = viewport.viewer.value?.THREE;
  const scale = object.scale.clone();
  const baked = object.userData.incrementalScale as number | undefined;
  if (THREE && typeof baked === "number" && Math.abs(baked) > 1e-8) {
    scale.divideScalar(baked);
  }
  return { position: object.position, quaternion: object.quaternion, scale };
}

function emitLiveTransform(object: ThreeNamespace.Object3D | null) {
  if (!object || typeof object.userData?.unitId !== "string") {
    emit("live-transform", null, null);
    return;
  }
  const THREE = viewport.viewer.value?.THREE;
  if (!THREE) return;
  const trs = unitSemanticTRS(object);
  const position = new THREE.Vector3();
  const quaternion = new THREE.Quaternion();
  const scale = new THREE.Vector3();
  new THREE.Matrix4()
    .compose(trs.position, trs.quaternion, trs.scale)
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
  const { TransformControls: Controls } =
    await import("three/examples/jsm/controls/TransformControls.js");
  if (viewport.viewer.value !== instance) return;
  const controls = new Controls(instance.camera, instance.domElement);
  controls.size = 0.85;
  controls.addEventListener("objectChange", () => {
    if (controls.dragging) {
      emitLiveTransform(controls.object);
      // 手柄拖拽直接改对象变换（不触发 grouping watcher），按需渲染下需显式重绘。
      viewport.viewer.value?.invalidate();
    }
  });
  controls.addEventListener("dragging-changed", (event) => {
    const dragging = (event as unknown as { value: boolean }).value;
    instance.setOrbitEnabled(!dragging);
    if (dragging) {
      const object = controls.object;
      // 与 commitGizmo 同语义（除掉烘焙 scale），否则无位移提交判等失败
      dragStartMatrix = object
        ? new instance.THREE.Matrix4().compose(
            object.position,
            object.quaternion,
            unitSemanticTRS(object).scale,
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
  const trs = unitSemanticTRS(object);
  const matrix = new THREE.Matrix4().compose(
    trs.position,
    trs.quaternion,
    trs.scale,
  );
  // 未产生位移的点击（拖拽起止矩阵相同）不产生冗余命令
  if (dragStartMatrix && matrix.equals(dragStartMatrix)) return;
  emit(
    "commit-transform",
    object.userData.unitId as string,
    threeToRowMajor(matrix),
  );
}

function updateGizmo() {
  const controls = gizmo;
  if (!controls) return;
  controls.detach();
  const tool = props.tool ?? "select";
  if (tool === "select" || !props.selectedId) {
    viewport.viewer.value?.invalidate();
    return;
  }
  const object = viewport.unitObjects.get(props.selectedId);
  if (!object) return;
  controls.setMode(tool);
  controls.attach(object);
  viewport.viewer.value?.invalidate();
}

/** 本次重建的触发来源，供渲染遥测标注（在 watcher 里按变化项判定）。 */
let pendingTrigger: RenderTelemetryTrigger = "first_load";

/** 贴花投影命中/回退计数（每次装配前重置），供 decal_render 遥测。 */
const decalStats = { projected: 0, fallback: 0, skipped: 0 };


/** 破洞族材质实例（2026-10-05 探针 hole_decal_dump 全量扫描定谳）：
 * 字典 0x1813DA18 的 material = 0x4491DE3A（decalInteriorMap /
 * shader 0x700000D2，docs/re/decal-engine-reanalysis-ida.md §11），
 * 29 个条目、194 个 decal 实例。早前"无 Color1-4 = 破洞"的判据在全库
 * 1694 条扫描中 **0 命中**（破洞条目带颜色字段）——后端 variant="hole"
 * 分支是死代码，破洞一直被错误路由到 clip 直采链（= "破洞是纯平斑块、
 * 无内景"的真正根因）。 */
const DECAL_HOLE_MATERIALS: ReadonlySet<number> = new Set([0x4491de3a]);

/** 本轮装配是否产生了 SDF 霓虹动画 decal（决定重建后是否启动动画时钟）。 */
let neonAnimated = false;
/** 霓虹动画时钟 rAF 句柄与上一帧时间戳。 */
let neonFrame = 0;
let neonLast = 0;
const particleViews: ParticleEffectView[] = [];
let particleFrame = 0;
let particleLast = 0;

function stopParticleClock() {
  if (particleFrame) cancelAnimationFrame(particleFrame);
  particleFrame = 0;
}

function clearParticleViews() {
  stopParticleClock();
  for (const view of particleViews) view.dispose();
  particleViews.length = 0;
}

function startParticleClock() {
  if (
    particleFrame ||
    !particleViews.length ||
    props.suspended ||
    props.renderMode !== "refined" ||
    !props.renderEffects ||
    props.groupVisibility.effects === false
  )
    return;
  particleLast = performance.now();
  const tick = (now: number) => {
    if (
      props.suspended ||
      props.renderMode !== "refined" ||
      !props.renderEffects ||
      props.groupVisibility.effects === false
    ) {
      stopParticleClock();
      return;
    }
    const delta = Math.min(0.1, (now - particleLast) / 1000);
    particleLast = now;
    for (const view of particleViews) view.tick(delta);
    viewport.viewer.value?.invalidate();
    particleFrame = requestAnimationFrame(tick);
  };
  particleFrame = requestAnimationFrame(tick);
}

/** 动态招牌开关状态由父组件 prop neonAnim 驱动（顶部工具条复选框）：
 * 默认关 = 静态量化合成招牌（uAnimEnabled=0）；开启后 SDF 族进入 dev
 * LED 扫掠动画并启动 rAF 时钟。 */
function applyNeonAnim(on: boolean) {
  if (envRefs) envRefs.animEnabled.value = on ? 1 : 0;
  if (on && neonAnimated) startNeonClock();
  else stopNeonClock();
  viewport.viewer.value?.invalidate();
}

watch(
  () => props.neonAnim,
  (on) => applyNeonAnim(on === true),
);

/**
 * 霓虹动画时钟（2026-10-05 六轮）：场景含 SDF 动画 decal 时以 rAF 推进
 * env.time（引擎 gameInfo.time 的墙钟近似）并逐帧 invalidate——viewer 是
 * 按需渲染底座，无动画场景不启动、零常驻开销。时钟对象由全部 SDF 材质
 * 共享引用（createEngineDecalMaterial 的 uTime 直引 env.time），推进即
 * 全场生效，无需遍历材质。
 */
function startNeonClock() {
  if (neonFrame || !envRefs) return;
  neonLast = performance.now();
  const tick = (now: number) => {
    neonFrame = requestAnimationFrame(tick);
    if (envRefs) envRefs.time.value += (now - neonLast) / 1000;
    neonLast = now;
    viewport.viewer.value?.invalidate();
  };
  neonFrame = requestAnimationFrame(tick);
}

function stopNeonClock() {
  if (neonFrame) cancelAnimationFrame(neonFrame);
  neonFrame = 0;
}

/** rebuild 包装：模型载荷身份变化时重新构图（编辑操作保持镜头）。 */
function rebuildScene() {
  clearParticleViews();
  const reframe = props.modelPayload !== lastPayload;
  lastPayload = props.modelPayload;
  // 只改 trigger：sessionKey 已由 usePropertyEditorSession 设好。
  renderTelemetry.setTrigger(pendingTrigger);
  pendingTrigger = "scene_rebuild";
  // 动画 decal 标志在装配期间由 buildDecalObject 置位；重建完成后按本轮
  // 结果启停霓虹时钟（新一代取代旧装配时同样以最新一轮为准）。
  return viewport
    .rebuild(assembleScene, { reframe })
    .then(() => {
      // env 每轮重建新建（animEnabled 归 0）→ 按开关状态重新应用；时钟
      // 只在"场景有 SDF 动画 decal 且开关开启"时运转。
      if (envRefs) envRefs.animEnabled.value = props.neonAnim === true ? 1 : 0;
      if (neonAnimated && props.neonAnim === true) startNeonClock();
      else stopNeonClock();
      startParticleClock();
    })
    .catch((error) => {
      // 装配异常显式化（2026-10-08 EP1 乱码取证）：异常若无声吞掉，场景
      // 会停在半装配状态（默认材质混杂）且断点不可见。
      console.error("[pe] 场景装配异常:", error);
    });
}

// ---- 重建合并调度（2026-10-07 性能轮）----
// 遥测实证：2154 次场景重建 / 682 次模型加载 ≈ 3.2×——模型载荷、grouping、
// 资产四通道（prop 模型/树模型/小人/直挂）各自触发全量重建，加载窗口内
// 邻帧到达即白跑 2-3 次（每次含全场景 dispose + 重装配 + 首帧编译）。
// 改为 rAF 单飞：同一帧内的多个触发合并为一次重建；触发语义按优先级
// 保留（lod_switch/first_load > render_mode > grouping）。
const TRIGGER_PRIORITY: Record<RenderTelemetryTrigger, number> = {
  first_load: 3,
  lod_switch: 3,
  render_mode: 2,
  grouping: 1,
  scene_rebuild: 0,
};
let rebuildScheduled = false;
let rebuildRequested = false;
function scheduleRebuild(trigger: RenderTelemetryTrigger) {
  if (props.suspended) return;
  if (
    !pendingTrigger ||
    TRIGGER_PRIORITY[trigger] >= TRIGGER_PRIORITY[pendingTrigger]
  ) {
    pendingTrigger = trigger;
  }
  rebuildRequested = true;
  if (rebuildScheduled) return;
  rebuildScheduled = true;
  requestAnimationFrame(async () => {
    try {
      while (rebuildRequested && !props.suspended) {
        await viewport.whenSettled();
        if (props.suspended) break;
        rebuildRequested = false;
        await rebuildScene();
      }
    } finally {
      rebuildScheduled = false;
    }
  });
}

/**
 * grouping 变化的**增量更新路径**：unit 集合不变、且只有非贴花 unit 的
 * transform 变化时，原地把新矩阵 decompose 进既有 Object3D——不重建任何
 * 几何/材质/贴花投影。此前拖拽手柄提交一次 transform、每次 undo/redo 都
 * 触发全量重建（2046ms p95 的直接来源）；增量路径预期 <5ms。
 *
 * 返回 false = 需全量重建（unit 增删、非 transform 字段变化、贴花 transform
 * ——投影几何随变换而变——或场景尚未装配）。
 */
function tryIncrementalGrouping(grouping: UnitGrouping): boolean {
  const instance = viewport.viewer.value;
  if (!instance) return false;
  if (!viewport.sceneReady.value) return false;
  const next = buildUnitsSnapshot(grouping);
  const prev = lastUnitsSnapshot;
  if (prev.size !== next.size) return false;
  let transformChanged = false;
  /** 仅 scale 变化的 unit（id → 新旧倍率）：免全量重建的热路径。 */
  const scaleRatios = new Map<string, number>();
  for (const [id, unit] of next) {
    const old = prev.get(id);
    if (!old) return false;
    if (old === unit) continue;
    if (!unitEqualsIgnoringTransform(old, unit)) {
      // 缩放倍率编辑（属性面板 scale 字段）：数字标注对象走增量比例；
      // keepScale（公告板/小人尺寸公式独立）与贴花投影几何仍须全量。
      const ratio = scaleOnlyDelta(old, unit);
      if (ratio === null || unit.kind === "decal") return false;
      const object = viewport.unitObjects.get(id);
      const flag = object?.userData.incrementalScale;
      if (flag === "keepScale") return false;
      if (typeof flag === "number") scaleRatios.set(id, ratio);
      // 无标注 = 矩阵语义（标记锥/灯等，scale 不参与渲染）→ 增量安全
      continue;
    }
    // 贴花投影几何在 lot 局部空间随 transform 而变，必须重建；
    // pathPoint 位置来自 point 字段（不消费 transform），无需应用。
    if (unit.kind === "decal") return false;
    transformChanged = true;
  }
  lastUnitsSnapshot = next;
  if (!transformChanged && scaleRatios.size === 0) return true;
  const THREE = instance.THREE;
  // 增量 transform 应用语义（渲染器在 userData.incrementalScale 标注）：
  // - 缺省 = 对象 scale 就是矩阵 decompose（标记锥/灯/贴花/占位人形）；
  // - 数字 = 渲染器把 DTO scale 烘焙进了对象 scale（树/prop 真模型），
  //   decompose 后必须补乘，否则 51m 原生巨树（真机勘误 2026-10-06）；
  //   伴随 scaleRatio（缩放倍率编辑）时再乘倍率；
  // - "keepScale" = 对象尺寸自持（公告板/合成小人），只应用位置/朝向。
  const scratchScale = new THREE.Vector3();
  for (const [id, unit] of next) {
    // pathPoint 位置来自 point 字段（不消费 transform）。
    if (unit.kind === "pathPoint" || !unit.transform) continue;
    const object = viewport.unitObjects.get(id);
    if (!object) continue;
    const incrementalScale = object.userData.incrementalScale as
      number | "keepScale" | undefined;
    unitMatrix(THREE, unit.transform).decompose(
      object.position,
      object.quaternion,
      incrementalScale === "keepScale" ? scratchScale : object.scale,
    );
    if (typeof incrementalScale === "number") {
      object.scale.multiplyScalar(incrementalScale);
      const ratio = scaleRatios.get(id);
      if (ratio !== undefined) {
        object.scale.multiplyScalar(ratio);
        // 烘焙系数随新倍率更新——下次增量 decompose/提交除回都用新值
        object.userData.incrementalScale = unitScaleValue(unit) ?? 1;
      }
    }
  }
  instance.invalidate();
  return true;
}

/** 量化 mask 图的尺寸（raw RGBA 字节流构造 ImageData 时需要宽高）。 */
async function loadMaskImageDims(): Promise<{
  width: number;
  height: number;
} | null> {
  const url = props.lotMaskPng ?? props.lotAlbedoPng;
  if (!url) return null;
  return cacheGetOrLoad(imageDimsCache, url, async () => {
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
  });
}

/** LotMask 原始通道权重图 → ImageData（compose 输入）。
 *  后端是未压缩 RGBA 字节流 base64（A = LC4 权重），**必须用 atob 直接构造
 *  ImageData**——若经 canvas 解码，预乘 alpha 会按 LC4 权重等比压缩/清零
 *  LC1-3 权重，精细合成随即满地判为草皮（2026-09-13 消防局对拍根因）。
 *  结果按源字符串缓存：编辑操作的全量重建不再重复 atob 逐字节拷贝。 */
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

/** "Lot Textures" 地表纹理 → 像素数据（compose v2 输入，按源 URL 缓存）。 */
function loadImageDataFromUrl(url: string | null): Promise<ImageData | null> {
  if (!url) return Promise.resolve(null);
  return cacheGetOrLoad(imageDataCache, url, () => decodeImageData(url));
}

function loadSurfacePixels(): Promise<ImageData | null> {
  const url = props.lotSurfacePng;
  if (!url) return Promise.resolve(null);
  // 地表图集（2048² 级）经 Worker 解码：主线程不再承担大图
  // drawImage+getImageData 的整块同步占用（OffscreenCanvas 轮）
  return cacheGetOrLoad(imageDataCache, url, () => decodePixelsForUrl(url));
}

onMounted(async () => {
  await viewport.ready;
  await rebuildScene();
  await ensureGizmo();
});
onBeforeUnmount(() => {
  stopNeonClock();
  clearParticleViews();
  gizmo?.detach();
  gizmo?.dispose();
  gizmo = null;
  // 跨 rebuild 缓存（模型克隆/材质贴图/地面合成/贴花纹理）**有意跨 unmount
  // 保留**：全部按 payload/session 身份自失效、容量有界，保留 = 重开同一
  // property 零解码秒开（GPU 缓冲随旧上下文销毁自动释放，仅剩 JS 侧数据）。
  imageDataCache.clear();
  imageDimsCache.clear();
  lastUnitsSnapshot.clear();
});

/** 业务场景装配：模型材质 → 地面 → 六类 Unit → 路径折线。 */

/** 让出一帧给浏览器（绘制/输入/合成）：装配全程 >1s（遥测 p95 ≈ 1s），
 * 不切片则输入与滚动在整段装配期间卡死。切片代价 = 多几次帧往返
 * （16.7ms/次），换整段可交互。 */
function yieldToBrowser(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

/** 资产渲染分发上下文（装配与部分刷新共用；每次调用按当前 props 取值）。 */
function buildAssetCtx(): UnitRenderContext {
  return {
    THREE: viewport.viewer.value!.THREE,
    renderMode: props.renderMode,
    treeModelPayloads: props.treeModelPayloads,
    treeAtlasPng: props.treeAtlasPng,
    treeIds: props.propTreeIds,
    propModels: props.propModels,
    addedModelPayloads: props.addedModelPayloads,
    envRefs,
    simParts: simParts.value,
  };
}

// ---- 资产通道部分刷新（2026-10-07 性能轮）----
// 遥测实证 2154 次场景重建 / 682 次加载：通道到位的"补渲染"此前走全量
// 重建——建筑/地面/贴花/路径全部推倒重来，实际只有 prop/spawner 两族
// 的呈现随载荷变化。改为只重渲染受影响族并原位换对象：
//   - 真实渲染器对 prop/spawner 是模板化快路径（GLB/材质模板缓存命中）
//   - 建筑网格、贴花投影、地面合成完全不动
// 守卫：suspended / 重建在飞（在飞装配自会消费新通道值）/ 跨 await 代数
// 变化（新一代已清场，塞回旧对象 = 幽灵对象）。
type AssetRefreshKind = "prop" | "spawner";
const pendingRefreshKinds = new Set<AssetRefreshKind>();
let refreshScheduled = false;
function scheduleAssetRefresh(kinds: AssetRefreshKind[]) {
  if (props.suspended) return;
  if (viewport.isAssembling()) return; // 在飞装配读的就是新通道值
  for (const kind of kinds) pendingRefreshKinds.add(kind);
  if (refreshScheduled) return;
  refreshScheduled = true;
  requestAnimationFrame(() => {
    refreshScheduled = false;
    const kinds = [...pendingRefreshKinds];
    pendingRefreshKinds.clear();
    void refreshAssetUnits(kinds);
  });
}
async function refreshAssetUnits(kinds: AssetRefreshKind[]) {
  const instance = viewport.viewer.value;
  if (!instance || props.suspended || !kinds.length) return;
  const epoch = viewport.rebuildEpoch();
  const assetCtx = buildAssetCtx();
  const span = renderTelemetry.begin("scene_rebuild", {
    refresh: kinds.join("+"),
  });
  try {
    for (const kind of kinds) {
      const family =
        kind === "prop" ? props.grouping.props : props.grouping.spawners;
      for (const unit of family) {
        if (viewport.rebuildEpoch() !== epoch) return; // 新一代已开跑
        const id = unitId(unit);
        const previous = viewport.unitObjects.get(id);
        if (!previous) continue; // 尚未装配（首建路径），重建自会处理
        const object =
          (await renderRefinedAssetUnit(assetCtx, unit)) ??
          buildUnitObject(assetCtx.THREE, unit);
        if (!object) continue;
        if (viewport.rebuildEpoch() !== epoch) {
          disposeObject(object);
          return;
        }
        object.userData.unitId = id;
        object.userData.unitKind = unit.kind;
        object.visible = !props.hiddenUnits.has(id);
        instance.group(kindGroup(kind)).add(object);
        viewport.unitObjects.set(id, object);
        instance.group(kindGroup(kind)).remove(previous);
        disposeObject(previous);
      }
    }
  } finally {
    span.end();
  }
  // 刷新的单元对象已换代：选中/描边/手柄若指向旧对象需重挂
  applyOutlines();
  updatePickOverlay();
  if (props.selectedId) updateGizmo();
  instance.invalidate();
}

async function assembleScene(
  ctx: Parameters<Parameters<typeof viewport.rebuild>[0]>[0],
) {
  const { viewer: instance, THREE } = ctx;
  specUniformRefs.length = 0;

  const payload = props.modelPayload;
  /** 贴花投影目标：全部建筑网格（在 lot 局部空间为单位变换）。模型线
   * 填充，贴花线在汇合点后消费。 */
  const buildingMeshes: ThreeNamespace.Mesh[] = [];
  // 5d 日/夜环境共享 uniform（全部 tint 材质引用同一组对象）。
  // 必须立刻按当前时段求值：天空三段色与 uSkyLumRef（球面均值）都依赖它，
  // 否则首帧用的是 createSunEnv 的占位值（间接光会整体偏暗）。
  const env = createSunEnv(THREE);
  applySunEnv(env, timeOfDay(), props.powered);
  envRefs = env;
  // 阶段间让出一帧（2026-10-07 性能轮）：整段装配遥测 p95 ≈ 1s，全程
  // 不切片则输入/滚动全程卡死。每个让出点后必须查 isStale——让出期间
  // 新一代重建可能已开跑（rebuild 骨架在让出前已清场）。
  await yieldToBrowser();
  if (ctx.isStale()) return;

  // ---- 三线并发（2026-10-07 二轮）：模型/地面/单元互不依赖输出——
  // 原串行 await 链总时长 = 各环节之和（模型最快出画、灯光/道具/地面
  // 整段滞后的直接原因）；并行后总时长 ≈ 最慢环节。贴花投影依赖建筑
  // 网格（模型线产物）、尺度标注依赖模型 bbox + 地面矩阵 → 汇合点后
  // 串行收尾。每线各自保留 isStale 检查。

  // 模型线：GLB 解析 → 材质/tint → 延迟贴图 → 阴影。
  const modelStage = (async () => {
    // payload 级缓存：命中时返回 clone（共享几何），跳过 GLB parse。
    const modelObjects = payload ? await getLotModelObjects(payload) : [];
    if (ctx.isStale()) {
      for (const object of modelObjects) disposeObject(object);
      return;
    }
    await executeModel({
      THREE,
      env,
      payload,
      modelObjects,
      renderMode: props.renderMode,
      modelSource: props.modelSource ?? "native",
      reliefEnabled: props.reliefEnabled === true,
      modelSelectable: props.modelSelectable === true,
      modelTransform: props.modelTransform ?? null,
      modelOrigin: props.modelOrigin ?? null,
      maxAnisotropy: ctx.maxAnisotropy,
      isStale: ctx.isStale,
      specUniforms: specUniformRefs,
      buildingMeshes,
      unitObjects: ctx.unitObjects,
      modelGroup: instance.group("model"),
      setShadowsEnabled: (enabled) => instance.setShadowsEnabled(enabled),
      applySun,
      invalidate: () => instance.invalidate(),
    });
  })();

  // 地面线：LotSize 矩形 + placement 逆 + mask 合成（与模型零依赖）。
  // 尺度标注依赖模型 bbox（模型线产物）→ 汇合点后再建。
  const groundStage = props.lotSize
    ? stageGround({
        THREE,
        lotSize: props.lotSize,
        lotPlacement: props.lotPlacement,
        lotOverlayBoxOffset: props.lotOverlayBoxOffset,
        lotModelBboxCenter: props.lotModelBboxCenter,
        renderMode: props.renderMode,
        lotMaskPng: props.lotMaskPng,
        lotAlbedoPng: props.lotAlbedoPng,
        lotNormalAtlasPng: props.lotNormalAtlasPng,
        lotSurfacePng: props.lotSurfacePng,
        lotMaskRawRgba: props.lotMaskRawRgba,
        lotTilePeriod: props.lotTilePeriod,
        lotColors: props.lotColors,
        lotBorderColors: props.lotBorderColors,
        lotBorderPatternIndices: props.lotBorderPatternIndices,
        lotBorderWidths: props.lotBorderWidths,
        lotColorHeights: props.lotColorHeights,
        lotBorderHeights: props.lotBorderHeights,
        lotBaseTile: props.lotBaseTile,
        groupLot: instance.group("lot"),
        primeTexture: (texture) => instance.primeTexture(texture),
        loadSurfacePixels,
        loadMaskImageDims,
        loadImageDataFromUrl,
        loadRawMaskPixels,
        isStale: ctx.isStale,
      })
    : ((dimLabels.value = []), Promise.resolve(null));

  /** 贴花装配状态（跨 decal 调用累计；每次 rebuild 前重置）。 */
  const decalExecState = {
    projected: 0,
    fallback: 0,
    holeLightCount: 0,
    neonAnimated: false,
  };

  async function buildDecalObject(
    THREE: typeof ThreeNamespace,
    unit: DecalUnit,
    texture: DecalUnitTexture,
    _meshes: ThreeNamespace.Mesh[],
    proxies: ThreeNamespace.Mesh[],
  ): Promise<ThreeNamespace.Object3D | null> {
    if (!texture.png) {
      console.warn(
        "[decal] DTO 无 png：",
        texture.idInstance,
        texture.error ?? "unknown",
      );
      return null;
    }
    const decoded = await getDecalTexture(THREE, texture);
    if (!decoded) return null;
    // 执行器自持一份状态，调用后回写到视口既有计数/时钟变量（保持旧语义）
    const state = { ...decalExecState };
    const object = await buildDecalBody({
      THREE,
      unit,
      texture,
      decoded,
      proxies,
      env,
      engineMaterials: true,
      decalLight: props.decalLight ?? null,
      powered: props.powered,
      isStale: ctx.isStale,
      state,
    });
    decalExecState.projected = state.projected;
    decalExecState.fallback = state.fallback;
    decalExecState.holeLightCount = state.holeLightCount;
    decalExecState.neonAnimated = state.neonAnimated;
    decalStats.projected = state.projected;
    decalStats.fallback = state.fallback;
    neonAnimated = state.neonAnimated;
    return object;
  }

  const units: LotUnitDto[] = [
    ...props.grouping.lights,
    ...props.grouping.decals,
    ...props.grouping.props,
    ...props.grouping.effects,
    ...props.grouping.spawners,
    ...props.grouping.pathPoints,
  ];
  // 精细模式贴花：category+index → 解码纹理（后端已按 ID 查 atlas 条目）。
  const decalTextureByKey = new Map(
    props.decalTextures.map((texture) => [
      `${texture.category}:${texture.index}`,
      texture,
    ]),
  );
  // 贴花解码纹理并行预取（去重后一次解码全部；此前逐 decal 串行 await，
  // 首载成本 = 贴花数 × 单张解码）——与模型/地面线并发（只依赖 props）。
  const decalPrefetch =
    props.renderMode === "refined" && decalTextureByKey.size
      ? Promise.all(
          [...new Set(decalTextureByKey.values())].map((texture) =>
            getDecalTexture(THREE, texture),
          ),
        )
      : Promise.resolve();
  // 资产渲染分发上下文（tree/prop/spawner 渲染器只读输入；每轮重建刷新）
  const assetCtx = buildAssetCtx();
  // 单元线：非贴花单元先行（灯光/道具/spawner/路径点，与模型/地面并发）；
  // 贴花收集到汇合点后投影（依赖建筑网格 = 模型线产物）。
  const decalUnits = units.filter(
    (unit): unit is DecalUnit => unit.kind === "decal",
  );
  const nonDecalUnits = units.filter((unit) => unit.kind !== "decal");
  await stageUnits({
    THREE,
    units: nonDecalUnits,
    renderMode: props.renderMode,
    renderEffects: props.renderEffects === true,
    effectDefinitions: props.effectDefinitions,
    effectsCount: props.grouping.effects.length,
    assetCtx,
    particleViews,
    createParticleEffect,
    renderAssetUnit: renderRefinedAssetUnit,
    buildFallback: buildUnitObject,
    buildLightUnit: buildRealLightUnit,
    groupFor: (name) => instance.group(name),
    unitObjects: ctx.unitObjects,
    unitIdOf: unitId,
    isStale: ctx.isStale,
    yieldToBrowser,
  });
  if (ctx.isStale()) return;

  // ---- 汇合点：三线全部落地（或已因过期自行退出）----
  await modelStage;
  if (ctx.isStale()) return;
  // Unit lighting is now complete and pruned. Compile stable model materials
  // concurrently with the independent ground worker; never compile a ground
  // placeholder that applyGroundMask is about to replace and dispose.
  const stableShaders = ctx.prepareStableShaders();
  const ground = await groundStage;
  await decalPrefetch;
  await stableShaders;
  if (ctx.isStale()) return;

  // Lot 三维尺度标注（用户需求 2026-10-06）：长/宽取地面矩形两邻边，
  // 高取建筑 bbox 实际高度（世界系 Y 跨度；世界仅旋转，跨度=游戏系 Z）。
  if (props.lotSize && ground && !props.groundOnly) {
    instance.scene.updateMatrixWorld(true);
    const modelBox = new THREE.Box3().setFromObject(instance.group("model"));
    const lotHeight = modelBox.isEmpty()
      ? 0
      : Math.max(0, modelBox.max.y - modelBox.min.y);
    const dimsPlan = lotDimensionPlan(props.lotSize, lotHeight);
    const dims = dimsPlan ? createLotDimensions(THREE, dimsPlan) : null;
    if (dims) {
      // 与地面矩形同一矩阵（placement 逆 + overlay 中心偏移），标注贴地边
      dims.object.matrix.copy(ground.matrix);
      dims.object.matrixAutoUpdate = false;
      instance.group("dimensions").add(dims.object);
      dimLabels.value = dims.anchors.map((anchor) => ({
        ...anchor,
        game: new THREE.Vector3(...anchor.point)
          .applyMatrix4(dims.object.matrix)
          .toArray() as [number, number, number],
      }));
    } else {
      dimLabels.value = [];
    }
  }

  // 贴花投影（依赖建筑网格 = 模型线产物；纹理已在预取线解码）。
  const decalSpan = renderTelemetry.begin("decal_render", {
    decals: decalUnits.length,
  });
  decalStats.projected = 0;
  decalStats.fallback = 0;
  neonAnimated = false;
  decalExecState.projected = 0;
  decalExecState.fallback = 0;
  decalExecState.holeLightCount = 0;
  decalExecState.neonAnimated = false;
  // 吸附射线目标：建筑网格代理（quad 摆放用）
  const decalProxies =
    props.renderMode === "refined" && buildingMeshes.length
      ? buildingMeshes.map((mesh) => new THREE.Mesh(mesh.geometry))
      : [];
  await stageDecals({
    THREE,
    renderMode: props.renderMode,
    decalUnits,
    decalTextureByKey,
    buildingMeshes,
    proxies: decalProxies,
    buildDecalObject,
    buildFallback: buildUnitObject,
    groupFor: (name) => instance.group(name),
    unitObjects: ctx.unitObjects,
    unitIdOf: unitId,
    isStale: ctx.isStale,
    onMissingTexture: (unit, keys) => {
      console.warn(
        `[decal] 配对失败 cat${unit.category}:idx${unit.index}，已注册键：`,
        keys,
      );
    },
  });

  decalSpan.end({
    projected: decalStats.projected,
    fallback: decalStats.fallback,
    skipped: decalStats.skipped,
    units: decalUnits.length,
  });

  // 路径折线：按 point_index 排序连接（pathPairs 语义未定，先 best-effort）。
  const points = [...props.grouping.pathPoints]
    .filter((point) => point.point)
    .sort((a, b) => (a.pointIndex ?? a.index) - (b.pointIndex ?? b.index))
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
  // 规模统计 → scene_rebuild 遥测 metadata（量化「property 规模 ↔ 耗时」）。
  ctx.stats.units = units.length;
  ctx.stats.decals = props.grouping.decals.length;
  ctx.stats.materials = payload?.materials?.length ?? 0;
  ctx.stats.meshes = payload?.glbs.length ?? 0;
  // 全量装配完成：刷新增量判定快照（与本次装配的 grouping 一致）。
  lastUnitsSnapshot = buildUnitsSnapshot(props.grouping);
  // 【关键不变量 2026-10-27】装配尾部强制整棵场景矩阵刷新后才能放行
  // 首帧：渐进装配/软跟随/闭 matrixAutoUpdate(地面/标注) 的组合下,首帧
  // 若撞上陈旧 matrixWorld,建筑与网格/标注的相对关系整体错乱(2026-10-27
  // 取证日志的 updateMatrixWorld 曾顺带掩盖此窗口,门控后立刻复现)。
  // 取证表格本身由 localStorage.openscpPeDebug 门控,与此刷新无关。
  instance.scene.updateMatrixWorld(true);
  if (import.meta.env.DEV && localStorage.getItem("openscpPeDebug") === "1") {
    instance.scene.updateMatrixWorld(true);
    const rows = VIEWPORT_GROUPS.map((name) => {
      const group = instance.group(name);
      const groupBox = new THREE.Box3().setFromObject(group);
      return {
        组: name,
        可见对象: group.children.length,
        min: groupBox.isEmpty() ? null : groupBox.min.toArray().map((v) => +v.toFixed(2)),
        max: groupBox.isEmpty() ? null : groupBox.max.toArray().map((v) => +v.toFixed(2)),
      };
    });
    console.group(`[pe-frame] rebuild#${ctx.token} 装配完成取证`);
    console.table(rows.filter((row) => row.min !== null));
    console.log(
      "content.position:",
      instance.contentPosition.toArray().map((v) => +v.toFixed(2)),
      "grid.y:",
      instance.gridY,
      "frameRadius:",
      instance.framingRadius,
      "ground.matrix:",
      ground?.matrix.elements.map((v) => +v.toFixed(2)),
      "lotPlacement:",
      props.lotPlacement,
    );
    for (const [id, unitObject] of ctx.unitObjects) {
      const unitBox = new THREE.Box3().setFromObject(unitObject);
      if (unitBox.isEmpty()) continue;
      const size = unitBox.getSize(new THREE.Vector3());
      const low = unitBox.min.y < -5;
      console.log(
        `${low ? "⚠️低位" : "  "} unit ${id}: min=${unitBox.min.toArray().map((v) => +v.toFixed(1))} size=${size.toArray().map((v) => +v.toFixed(1))}`,
      );
    }
    console.groupEnd();
  }
}

/** 日/夜亮度：仅缩放环境四灯；地块真实光源保持常亮（夜间灯依然亮）。 */
function applyBrightness() {
  // 5d：夜间环境光随白昼因子压暗（亮度滑杆仍是用户侧总倍率）
  viewport.viewer.value?.setEnvironmentBrightness(
    brightness.value * (0.22 + 0.78 * dayFactor(timeOfDay())),
  );
}

defineExpose({
  /**
   * 渲染图导出（2026-10-08 修复）：尺度数值标签是 HTML 浮层，不进
   * WebGL 画布——截图后按当前投影把标签补绘到 2D 画布再导出，否则
   * 只截到尺度线、截不到数值。
   */
  captureRender: async (options?: {
    includeDecals?: boolean;
  }): Promise<string | null> => {
    const dataUrl = viewport.captureRender(options);
    if (!dataUrl) return null;
    const instance = viewport.viewer.value;
    if (!instance || !dimLabels.value.length) return dataUrl;
    // 尺度组隐藏（图层开关）时不补标签
    if (props.groupVisibility.dimensions === false) return dataUrl;
    try {
      const image = await new Promise<HTMLImageElement>((resolve, reject) => {
        const element = new Image();
        element.onload = () => resolve(element);
        element.onerror = () => reject(new Error("capture load failed"));
        element.src = dataUrl;
      });
      const rect = viewport.container.value?.getBoundingClientRect();
      // 截图像素 = CSS 像素 × DPR：标签按同一比例缩放对位
      const ratio = rect?.width ? image.naturalWidth / rect.width : 1;
      const canvas = document.createElement("canvas");
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      const context = canvas.getContext("2d");
      if (!context) return dataUrl;
      context.drawImage(image, 0, 0);
      context.font = `600 ${Math.max(10, Math.round(12 * ratio))}px ui-sans-serif, system-ui, sans-serif`;
      context.textAlign = "center";
      context.textBaseline = "middle";
      context.fillStyle = "#0878fe";
      context.shadowColor = "rgba(0, 0, 0, 0.6)";
      context.shadowBlur = 2 * ratio;
      const axisLabels: Record<DimensionAnchor["axis"], string> = {
        length: t("package.dimLength"),
        width: t("package.dimWidth"),
        height: t("package.dimHeight"),
      };
      for (const dim of dimLabels.value) {
        const scenePoint = instance.world.localToWorld(
          new instance.THREE.Vector3(...dim.game),
        );
        const screen = instance.worldToScreen(scenePoint);
        if (screen.behind) continue;
        context.fillText(
          `${axisLabels[dim.axis]} ${dim.text}`,
          screen.x * ratio,
          screen.y * ratio,
        );
      }
      return canvas.toDataURL("image/png");
    } catch {
      return dataUrl;
    }
  },
  /** 屏幕坐标 → 地面平面交点（游戏坐标），组件库拖放放置用。 */
  groundPointAt: (clientX: number, clientY: number) =>
    viewport.viewer.value?.groundPointAt(clientX, clientY) ?? null,
});

// 模型载荷 / 渲染模式变化 → 全量重建；grouping 变化 → 先试增量（热路径），
// 失败（unit 增删/字段变化/贴花移动）才全量。同一 flush 内两者都变时
// （如会话加载），rebuildToken 保证后到者胜出。全部触发走 scheduleRebuild
// 合并（2026-10-07：原始实现各通道独立重建，3.2× 重复，见调度器注释）。
watch(
  () => [props.modelPayload, props.renderMode] as const,
  ([payload], previous) => {
    // 按变化项判定触发来源（首次拿到 payload 记 first_load，换级记 lod_switch）。
    const trigger: RenderTelemetryTrigger =
      previous?.[0] == null
        ? "first_load"
        : payload !== previous[0]
          ? "lod_switch"
          : "render_mode";
    scheduleRebuild(trigger);
  },
);
watch(
  () => [props.renderEffects, props.effectDefinitions] as const,
  () => {
    if (!props.renderEffects) {
      stopParticleClock();
      if (particleViews.length) scheduleRebuild("grouping");
    } else if (props.effectDefinitions?.size) {
      scheduleRebuild("grouping");
    }
  },
);
watch(
  () => props.suspended,
  (suspended) => {
    if (suspended) stopParticleClock();
    else startParticleClock();
  },
);
watch(
  () => props.grouping,
  (grouping) => {
    if (props.renderEffects && props.effectDefinitions?.size)
      scheduleRebuild("grouping");
    else if (!tryIncrementalGrouping(grouping)) scheduleRebuild("grouping");
  },
);
// Raster editing updates the ground independently of models and units.
watch(
  () => [
    props.lotMaskPng,
    props.lotMaskRawRgba,
    props.lotSurfacePng,
    props.lotNormalAtlasPng,
    props.lotColors,
    props.lotSize,
    props.lotTilePeriod,
  ],
  () => scheduleRebuild("grouping"),
  { deep: true },
);
watch(
  () => props.groupVisibility,
  () => {
    viewport.applyGroupVisibility(props.groupVisibility);
    if (props.groupVisibility.effects === false) stopParticleClock();
    else startParticleClock();
  },
  { deep: true },
);
// 异步资产通道到位 → **部分刷新**受影响族（prop/spawner 原位换对象），
// 不再整场重建（2026-10-07：此前四通道各触发一次全量重建，建筑/地面/
// 贴花全部白跑——遥测 2154 重建/682 加载 ≈ 3.2× 的主要成分）。初始重建
// 未就绪时 unitObjects 为空，刷新自然 no-op，由重建本身消费新载荷。
watch(
  () =>
    [
      props.propModels,
      props.treeModelPayloads,
      props.addedModelPayloads,
      simParts.value,
    ] as const,
  (next, previous) => {
    const kinds: AssetRefreshKind[] = [];
    if (
      next[0] !== previous[0] ||
      next[1] !== previous[1] ||
      next[2] !== previous[2]
    ) {
      kinds.push("prop");
    }
    if (next[3] !== previous[3]) kinds.push("spawner");
    if (kinds.length) scheduleAssetRefresh(kinds);
  },
);
// 单元隐藏此前只在装配期应用（无 watcher → 切换后无效果直到下次重建）；
// 现在独立生效。
watch(
  () => props.hiddenUnits,
  () => viewport.applyUnitVisibility(props.hiddenUnits),
);
// 挂起恢复（sheet 重开）→ 补一次重建对齐当前渲染模式（关闭期间模式被
// 复位为 default 而未重建）；挂起期间触发器静默（scheduleRebuild 入口
// 拦截），恢复后的这一次由本 watch 发起。
watch(
  () => props.suspended,
  (suspended, wasSuspended) => {
    if (wasSuspended && suspended === false) scheduleRebuild("render_mode");
  },
);
watch(
  () => props.selectedId,
  () => viewport.applySelection(props.selectedId),
);
// 工具/选中/rebuild 代数变化 → 重挂或摘除手柄（选中对象会被重建）
watch([() => props.tool, () => props.selectedId, viewport.revision], () =>
  updateGizmo(),
);
</script>

<template>
  <div class="viewport-pane">
    <div
      :ref="(el) => (viewport.container.value = el as HTMLElement | null)"
      class="viewport-3d"
    >
      <!-- 拾取反馈标签：轮廓壳在 viewer 内（模型描边），此处仅文字 -->
      <span
        :ref="(el) => (hoverTagEl = el as HTMLElement | null)"
        class="pick-label"
        aria-hidden="true"
      />
      <!-- 尺度标注文字（线段在 viewer dimensions 组，随帧投影居中锚定） -->
      <span
        v-for="(dim, index) in dimLabels"
        :key="dim.axis"
        :ref="(el) => (dimLabelEls[index] = el as HTMLElement | null)"
        class="pick-label dim-label"
        aria-hidden="true"
        >{{ dimAxisLabel[dim.axis] }} {{ dim.text }}</span
      >
      <span
        :ref="(el) => (selectTagEl = el as HTMLElement | null)"
        class="pick-label pick-tag"
        aria-hidden="true"
      >
        <span class="pick-tag-text" />
        <button
          v-if="props.editEnabled && props.selectedId !== 'model'"
          type="button"
          class="pick-delete"
          :aria-label="$t('package.deleteUnit')"
          @click.stop="onDeleteSelected"
        >
          <FIcon name="Trash2" :size="11" aria-label="" />
        </button>
      </span>
    </div>
    <div
      v-if="!groundOnly && modelState === 'loading'"
      class="viewport-overlay viewport-status"
    >
      <FSpinner size="sm" :label="$t('common.loading')" />
    </div>
    <p
      v-else-if="
        !groundOnly && (modelState === 'missing' || modelState === 'error')
      "
      class="viewport-overlay viewport-status"
      role="status"
    >
      {{
        modelState === "missing"
          ? $t("package.propertyEditorModelMissing")
          : $t("package.propertyEditorModelError")
      }}
    </p>
    <div v-if="!groundOnly" class="viewport-overlay viewport-tools">
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
            @click="
              emit('switch-lod', index);
              lodPanelOpen = false;
            "
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
        <input
          v-model.number="brightness"
          type="range"
          min="0"
          max="2"
          step="0.05"
        />
      </label>
    </div>
    <div v-if="infoPanelOpen" class="viewport-overlay info-panel">
      <div
        class="info-card"
        role="dialog"
        :aria-label="$t('package.modelInfo')"
      >
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
              <FIcon
                :name="infoCopied ? 'Check' : 'Copy'"
                :size="13"
                aria-label=""
              />
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
        <pre class="info-pre">{{
          modelPayload?.diagnostics || $t("package.modelInfoEmpty")
        }}</pre>
      </div>
    </div>
    <div
      v-if="props.editEnabled && !groundOnly"
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
      <!-- 动态招牌开关已挪入顶部工具条（供电旁复选框，2026-10-05 十轮） -->
    </div>
    <div
      v-if="!groundOnly"
      class="viewport-overlay viewport-visibility"
      role="group"
      :aria-label="$t('package.visibilityToggles')"
    >
      <button
        v-for="name in [
          'model',
          'lot',
          'dimensions',
          'lights',
          'props',
          'decals',
          'effects',
          'spawners',
          'paths',
        ]"
        :key="name"
        type="button"
        :class="{ off: groupVisibility[name] === false }"
        :aria-pressed="groupVisibility[name] !== false"
        :title="$t(`package.group${name[0].toUpperCase()}${name.slice(1)}`)"
        @click="$emit('toggle-layer', name)"
      >
        <FIcon
          :name="groupVisibility[name] === false ? 'EyeOff' : 'Eye'"
          :size="13"
          aria-label=""
        />
        <span>{{
          $t(`package.group${name[0].toUpperCase()}${name.slice(1)}`)
        }}</span>
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
  position: relative;
  touch-action: none;
}
/* 拾取反馈标签（轮廓壳在 viewer 内，模型描边）：锚定组件原点上方 */
.pick-label {
  color: var(--brand);
  display: none;
  font-size: 10px;
  line-height: 1;
  padding: 2px 4px;
  pointer-events: none;
  position: absolute;
  text-shadow: 0 1px 2px rgb(0 0 0 / 60%);
  transform: translateY(calc(-100% - 6px));
  white-space: nowrap;
  z-index: 5;
}
.pick-tag {
  background: var(--brand);
  border-radius: 3px;
  color: #fff;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding-inline-end: 3px;
  text-shadow: none;
}
/* 尺度标注标签：线段中点居中锚定（覆盖 pick-label 的上方偏移） */
.dim-label {
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  transform: translate(-50%, -50%);
}
.pick-delete {
  background: transparent;
  border: 0;
  border-radius: 2px;
  color: #fff;
  cursor: pointer;
  display: inline-flex;
  min-height: 16px;
  min-width: 16px;
  align-items: center;
  justify-content: center;
  padding: 0;
  pointer-events: auto;
}
.pick-delete:hover {
  background: rgb(255 255 255 / 25%);
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
  transition:
    border-color 120ms ease,
    box-shadow 120ms ease;
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
  /* 左上角是图层列表的地盘——工具栏放右侧垂直居中，避免重叠（用户指令
   * 2026-10-25：操作栏改到右边） */
  inset-inline-end: 10px;
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
