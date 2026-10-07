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
  LotModelLodRef,
  LotModelPayload,
  LotUnitDto,
} from "@/api/tauri";
import type { ModelState, UnitGrouping } from "./usePropertyEditorSession";
import { unitLabel } from "./usePropertyEditorSession";
import {
  buildPathLine,
  buildRealLightUnit,
  buildUnitObject,
  unitId,
  unitMatrix,
} from "./unitGizmos";
import { loadSimParts, type SimPart } from "./simAssets";
import {
  renderRefinedAssetUnit,
  type UnitRenderContext,
} from "./renderers";
import {
  decalFrame,
  decalHalfThickness,
} from "@/lib/decalProject";
import type { DecalFrame } from "@/lib/decalProject";
import { createEngineDecalMaterial } from "@/lib/decalEngineMaterials";
import type { EngineFamily } from "@/lib/decalEngineMaterials";
import { useEditorViewport } from "./useEditorViewport";
import {
  applyDeferredMaterialMaps,
  applySunEnv,
  createSunEnv,
  dayFactor,
  getDeferredMaps,
  getTintTextures,
  makeTintMaterial,
  type SunEnvRefs,
} from "./refinedRender";
import { threeToRowMajor } from "./unitEditLayer";
import { installHejlToneMapping } from "@/lib/hejlTonemapping";
import type { TransformControls } from "three/examples/jsm/controls/TransformControls.js";
import {
  applyGroundMask,
  buildLotDimensions,
  buildLotRect,
  groundFillMesh,
  placementInverse,
  type LotDimensionAnchor,
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
  /** 边框带图案索引（LotBorderColor.A，0-15）。 */
  lotBorderPatternIndices: number[];
  /** 底图格索引（后端三级来源：0x0CCB7FD6 → 推导 → 8）。 */
  lotBaseTile: number;
  /** LotOverlayBoxOffset（0x0CCB7FC9，引擎 unitOffset）：地面 quad 在 lot
   * 系的中心偏移；null = 原点（引擎栅格公式缺省）。 */
  lotOverlayBoxOffset: [number, number] | null;
  /** Model Bounding Box（0x00F9EFBA）的 xy 中心（模型空间）。【2026-10-05
   * 试点后不再消费】：引擎栅格映射无 bbox 中心机制（该属性只喂 zoning
   * frontage/depth 截断），保留 DTO 仅供诊断对照。 */
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
  /** 动态招牌开关（默认 false = 静态量化合成招牌）：开启后 SDF 族进入
   * dev LED 扫掠动画并启动 rAF 时钟。顶部工具条复选框驱动（2026-10-05
   * 十轮：原视口 ⚡ 按钮用户找不到，挪入顶部工具条"供电"旁）。 */
  neonAnim?: boolean;
  /** 破洞贴花假内景光参数 [光强因子, 半径因子]（0x0DA76A05/06）；缺失 = 无。 */
  decalLight?: [number, number] | null;
  /** 当前编辑工具（select = 仅拾取；其余挂 TransformControls 手柄）。 */
  tool?: EditorTool;
  /** 编辑模式（解锁）：工具栏可见、画布可拖放、name-tag 带删除钮。 */
  editEnabled?: boolean;
  /** 拖入的直挂模型载荷（instance → LOTM 载荷；真模型渲染用）。 */
  addedModelPayloads?: Map<number, LotModelPayload>;
  /** 放置后待选中：分组重建且新单元入组时自动选中（shell 传入）。 */
  pendingSelectId?: string | null;
  /**
   * 挂起（PE sheet 关闭中）：全部重建触发器静默——关闭时 shell 会复位
   * renderMode，不挂起就会在卸载 EcoGame 包的同时触发一次完整重建
   * （关闭卡顿根因，2026-10-07）。恢复可见时由 shell 翻回 false，
   * 视口补一次重建对齐当前渲染模式。
   */
  suspended?: boolean;
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
  void loadSimParts().then((parts) => {
    simParts.value = parts;
  });
});
const viewport = useEditorViewport({
  onTapUnit: (id) => emit("select", id),
  onHoverUnit: (id) => {
    hoveredId.value = id;
  },
});

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
  (LotDimensionAnchor & { game: [number, number, number] })[]
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
      ? viewport.unitObjects.get(hover) ?? null
      : null;
  instance.setHoverOutline(hoverObject);
  const selected = props.selectedId;
  instance.setSelected(
    selected ? viewport.unitObjects.get(selected) ?? null : null,
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
const imageDimsCache = new Map<string, Promise<{ width: number; height: number } | null>>();

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
    const decoded = await new THREE.TextureLoader().loadAsync(
      `data:image/png;base64,${texture.png}`,
    ).catch((error: unknown) => {
      console.warn("[decal] 纹理 data URL 解码失败", texture.idInstance, error);
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
 * A/B 开关：true = decal 材质走引擎 GLSL 管线（sc-shader 产物，
 * crates/sc-shader 组合器生成，逐字对齐引擎片段公式）；false = 手写材质。
 * 对拍结论留任一方后删除另一方。
 */
const ENGINE_SHADER_MATERIALS = true;

/**
 * 上一次装配/增量应用的 unit DTO 快照（id → unit）：增量更新判定的对照物。
 * 全量 rebuild 收尾与增量应用成功后都会刷新。
 */
let lastUnitsSnapshot = new Map<string, LotUnitDto>();

function buildUnitsSnapshot(
  grouping: UnitGrouping,
): Map<string, LotUnitDto> {
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
function unitEqualsIgnoringTransform(
  a: LotUnitDto,
  b: LotUnitDto,
): boolean {
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

/** 破洞假内景光单次装配上限（防多破洞 lot 光源洪峰）。 */
const HOLE_LIGHT_MAX = 8;
let holeLightCount = 0;

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
  const reframe = props.modelPayload !== lastPayload;
  lastPayload = props.modelPayload;
  // 只改 trigger：sessionKey 已由 usePropertyEditorSession 设好。
  renderTelemetry.setTrigger(pendingTrigger);
  // 动画 decal 标志在装配期间由 buildDecalObject 置位；重建完成后按本轮
  // 结果启停霓虹时钟（新一代取代旧装配时同样以最新一轮为准）。
  return viewport.rebuild(assembleScene, { reframe }).then(() => {
    // env 每轮重建新建（animEnabled 归 0）→ 按开关状态重新应用；时钟
    // 只在"场景有 SDF 动画 decal 且开关开启"时运转。
    if (envRefs) envRefs.animEnabled.value = props.neonAnim === true ? 1 : 0;
    if (neonAnimated && props.neonAnim === true) startNeonClock();
    else stopNeonClock();
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
function scheduleRebuild(trigger: RenderTelemetryTrigger) {
  if (props.suspended) return;
  if (
    !pendingTrigger ||
    TRIGGER_PRIORITY[trigger] >= TRIGGER_PRIORITY[pendingTrigger]
  ) {
    pendingTrigger = trigger;
  }
  if (rebuildScheduled) return;
  rebuildScheduled = true;
  requestAnimationFrame(() => {
    rebuildScheduled = false;
    void rebuildScene();
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
      | number
      | "keepScale"
      | undefined;
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
function loadImageDataFromUrl(
  url: string | null,
): Promise<ImageData | null> {
  if (!url) return Promise.resolve(null);
  return cacheGetOrLoad(imageDataCache, url, () => decodeImageData(url));
}

function loadSurfacePixels(): Promise<ImageData | null> {
  const url = props.lotSurfacePng;
  if (!url) return Promise.resolve(null);
  return cacheGetOrLoad(imageDataCache, url, () => decodeImageData(url));
}

onMounted(async () => {
  await viewport.ready;
  await rebuildScene();
  await ensureGizmo();
});
onBeforeUnmount(() => {
  stopNeonClock();
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
      const family = kind === "prop" ? props.grouping.props : props.grouping.spawners;
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
  const tintSpan = renderTelemetry.begin("texture_compose", {
    phase: "tint",
    refined: props.renderMode === "refined",
  });
  const tintResolved =
    props.renderMode === "refined" && payload
      ? await getTintTextures(THREE, payload, ctx.maxAnisotropy)
      : [];
  tintSpan.end({
    materials: payload?.materials?.length ?? 0,
    tinted: tintResolved.filter((entry) => entry?.tintTex).length,
  });
  if (ctx.isStale()) return;
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
      // 贴花投影的候选面（精细模式才用；建筑网格在 lot 局部空间为单位变换）
      buildingMeshes.push(mesh);
      // 记录 mesh 所属材质列：破洞内景需取同材质的 slot5 房间图集
      mesh.userData.buildingMaterialIndex = materialIndex;
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
    const deferredSpan = renderTelemetry.begin("texture_compose", {
      phase: "deferred",
    });
    // await 全部解码完成再应用：真实成本进 span（此前只覆盖同步签发）；
    // 缓存命中时 await 即时返回。
    const deferredMaps = await getDeferredMaps(THREE, payload, ctx.maxAnisotropy);
    if (!ctx.isStale()) {
      applyDeferredMaterialMaps(deferredMaps, materialGroups);
      instance.invalidate();
    }
    deferredSpan.end({ groups: materialGroups.length });
  }
  // 阴影链（精细模式）：key 光投影 + 内容网格 castShadow（幂等，重建后
  // 新网格也补标）。太阳方向/色由 applySun 的 setSunFromEnv 持续驱动；
  // 此处重放一次 applySun 保证切换渲染模式后 env 太阳立即生效。
  const shadowsRefined = props.renderMode === "refined";
  viewport.viewer.value?.setShadowsEnabled(shadowsRefined);
  if (shadowsRefined) applySun();
  })();

  // 地面线：LotSize 矩形 + placement 逆 + mask 合成（与模型零依赖）。
  // 尺度标注依赖模型 bbox（模型线产物）→ 汇合点后再建。
  const groundStage = (async () => {
    if (!props.lotSize) {
      dimLabels.value = [];
      return null;
    }
    const groundSpan = renderTelemetry.begin("lot_render", {
      refined: props.renderMode === "refined",
    });
    const ground = buildLotRect(THREE, props.lotSize);
    // 关闭自动更新：矩阵完全由 placement 逆决定，防止渲染循环覆盖。
    if (props.lotPlacement) {
      ground.matrix.copy(placementInverse(THREE, props.lotPlacement));
    }
    // 【2026-10-05 试点：引擎栅格定位口径（dev-dump 文档 §四 F7）】
    // 地面 quad 在 lot 本地系的中心 = unitOffset（0x0CCB7FC9，缺省 0）——
    // 引擎 mask UV 公式 uv=(pos−unitOffset)/LotSize+0.5（migration §42.8）
    // 没有 bbox 中心机制；0x00F9EFBA 声明 bbox 在引擎只喂 zoning
    // frontage/depth 截断（GetUnitBoundingBoxInternal），不再作地面中心
    // fallback（图书馆 −0.41 / EP1 房 +2.89 残差的疑源）。
    // 偏移在 lot 系施加 = postmultiply（P⁻¹·T(c)）：θ=0 的 lot 与旧实现
    // 逐值等价（旧 premultiply T(A·c)·P⁻¹ 在 θ=0 时同为平移 c−t），θ≠0
    // 时修正偏移的旋转方向（旧 A·c 按 P 正向旋转，方向相反）。
    const centerLocal = props.lotOverlayBoxOffset ?? [0, 0];
    if (Math.abs(centerLocal[0]) > 1e-4 || Math.abs(centerLocal[1]) > 1e-4) {
      ground.matrix.multiply(
        new THREE.Matrix4().makeTranslation(centerLocal[0], centerLocal[1], 0),
      );
    }
    ground.matrixAutoUpdate = false;
    // 独立 lot 组：与建筑模型分开控制可见性（关模型不连地面一起隐藏，
    // 2026-09-13 用户对拍需求）。
    instance.group("lot").add(ground);
    if (props.lotMaskPng || props.lotAlbedoPng) {
      // v2：先加载地表纹理像素，失败/缺失时 compose 回退 v1。
      // applyGroundMask 已 await（compose 成本进 scene_rebuild 遥测；
      // 解码结果按源字符串缓存，编辑操作的全量重建零重复解码）。
      const [surface, maskDims, normalAtlas] = await Promise.all([
        loadSurfacePixels(),
        loadMaskImageDims(),
        loadImageDataFromUrl(props.lotNormalAtlasPng),
      ]);
      if (!surface) {
        // 精细渲染的底图格依赖真实图集；静默回退占位 tile 会把沥青画成
        // 亮灰（2026-09-13 对拍教训），必须让用户看到原因。
        console.warn(
          "[lot-ground] 'Lot Textures' surface unavailable — refined ground will use placeholder tiles. Open SimCity_Graphics.package (and the lot's own package) for the real atlas.",
        );
      }
      const rawMask = maskDims
        ? loadRawMaskPixels(maskDims.width, maskDims.height)
        : null;
      await applyGroundMask({
        rawMask,
        surface,
        normalAtlas,
        THREE,
        ground,
        maskPng: props.lotMaskPng,
        albedoPng: props.lotAlbedoPng,
        lotSize: props.lotSize,
        tilePeriod: props.lotTilePeriod,
        refined: props.renderMode === "refined",
        lotColors: props.lotColors,
        lotBorderColors: props.lotBorderColors,
        lotBorderPatternIndices: props.lotBorderPatternIndices,
        lotBorderWidths: props.lotBorderWidths,
        baseTileIndex: props.lotBaseTile,
        lotOverlayBoxOffset: props.lotOverlayBoxOffset,
        rawMaskKey: props.lotMaskRawRgba,
        surfaceKey: props.lotSurfacePng,
        normalAtlasKey: props.lotNormalAtlasPng,
        isStale: ctx.isStale,
      });
    }
    groundSpan.end({ masked: Boolean(props.lotMaskPng || props.lotAlbedoPng) });
    // 地面合成产物提前上传 GPU：2048² +mipmap 的上传是首帧大冻结源，
    // 放在地面线内 = 与模型/单元线并发吸收（不占汇合后的首帧）。
    const fill = groundFillMesh(ground);
    const fillMap = (fill?.material as ThreeNamespace.Material | undefined)
      ? ((fill!.material as ThreeNamespace.MeshLambertMaterial).map ?? null)
      : null;
    if (fillMap) instance.primeTexture(fillMap);
    return ground;
  })();

  /**
   * 贴花材质变体（migration.md §52.5 聚类映射）：招牌 = 霓虹自发光
   * （decalNeonBrighten 证据：Current.color.rgb ×2，无光照、夜间自亮）；
   * 涂鸦/废墟 = 受光材质——MeshBasic 不受光导致贴花与墙面的光照/明暗完全
   * 脱节（"不在一个图层"观感的根因），MeshStandard 融入场景光照。
   */
  /**
   * Decal 材质子类型（migration.md §56-59）：引擎 decal PS 全家族都是
   * **直采 raster + 标准 alpha 混合**（`Current.color = tex2D(s0, uv)`），
   * 子类型差异只在增亮/光照/动画分支。raw 纹理的 alpha 本身就是美术授权
   * 的低不透明度（字母 0.7-0.86、底色 <0.2）——「涂鸦刷进墙里」的效果
   * = 低 alpha 叠加 + 共享光照，不是屏幕域乘法（§58 的 DstColor 调制
   * 让暗底 texel 把整块墙压暗、字也没了，已回退）。
   * - sign（0x73684EFC）：霓虹自发光，×2 增亮（decalFloatQuadNoClip）；
   * - graffiti（0xE5390A98，cGraphicsUnitVandalism）：×1 直采 alpha 混合；
   * - 其余（未知材质兜底）：受光 MeshStandard。
   */
  /**
   * 混合分派（2026-10-01 二次修正）：
   * - alpha 有梯度（喷漆/柔边）：引擎混合态 (SRCALPHA, DESTALPHA)——
   *   out = 贴图RGB×A + 底色，字母 a≈0.7 → 30% 透墙（游戏观测一致）；
   * - alpha 全零实心图（海报式，自带背景色）：**不透明渲染**——它们没有
   *   裁剪信息，叠加混合会把背景色与墙色双份叠加成半透洗白（上轮教训）。
   */
  function applyEngineBlend(
    material: ThreeNamespace.Material,
    dto: DecalUnitTexture,
  ): void {
    // 量化合成产物 = 硬 alpha 裁剪（层内 255/层外 0）——标准混合即正确
    material.blending = THREE.NormalBlending;
    material.transparent = true;
    void dto;
  }

  const DECAL_MATERIAL_VARIANTS: Record<number, "sign" | "graffiti"> = {
    0x73684efc: "sign", // 招牌聚类（POWER ELECTRIC/太阳 burst/OMEGACO 等）
    0xe5390a98: "graffiti", // 涂鸦/贴纸聚类（CRIME/词组拼贴等）
  };

  /** 量化合成（quant）族材质集合（2026-10-04 三轮用户对拍定谳）：字典
   * 两族都走 0.5 阈值多通道合成——涂鸦的"清晰图案"正是多通道权重经
   * 阈值链上色的产物（误路由到直采 = 权重通道当颜色 → 彩色模糊涂抹，
   * 三轮图4）；焦痕/烧灼与未知材质走 clip 直采（raster RGB = 美术内容，
   * 误走量化 = 层色近黑整块纯黑，二轮图1~3）。 */
  const DECAL_QUANT_MATERIALS: ReadonlySet<number> = new Set([
    0x73684efc, 0xe5390a98,
  ]);

  /** 投影面法线 cutoff：只保留 **N·axisZ ≤ -0.5** 的三角形——即面朝贴花
   * 原点（投影来向）的面。两个裁剪目标：
   * 1) |N·axisZ| < 0.5 的切向面被盒体裁剪后 UV 极度拉伸 = 立面"线性涂抹"
   *    （三轮图2）；
   * 2) N·axisZ > 0 的**远侧面**（薄板结构背坡、建筑背面外墙——外向法线
   *    背对原点）= "穿透到另一侧"镜像字（五轮图1~2）。引擎延迟投影逐像素
   *    取最近深度、天然只画最近面，CPU 几何投影必须同时满足这两个条件
   *    才能同构（60° 以内朝向原点的曲面绕折仍保留）。 */
  const DECAL_PROJECTION_NORMAL_CUTOFF = 0.5;

  /** 浮空族判据：sign 字典 decal 的 materialData[1] 下限（≥ 此值 → 引擎
   * FloatQuad 族，画数据位姿浮空 quad 而非体积投影）。实证：casino 墙
   * 招牌 0 / 高塔竖幅 0.95~1.0 / DIRTY FACTORY 0.06 / 涂鸦恒 0。
   * 语义定谳（§十五）：该分量 = animSpeed 跑马灯速度。 */
  const DECAL_FLOAT_EMISSIVE_CUTOFF = 0.9;

  /**
   * 按面法线过滤投影几何：只保留面朝贴花原点的三角形。返回 null = 整盒
   * 无保留面（按未命中处理）。
   */
  function filterDecalProjectionByNormal(
    THREE: typeof ThreeNamespace,
    geometry: ThreeNamespace.BufferGeometry,
    axisZ: ThreeNamespace.Vector3,
  ): ThreeNamespace.BufferGeometry | null {
    const pos = geometry.attributes.position;
    const nrm = geometry.attributes.normal;
    const uv = geometry.attributes.uv;
    if (!pos || !nrm || !uv) return geometry; // 无法线不可判，原样保留
    const positions: number[] = [];
    const normals: number[] = [];
    const uvs: number[] = [];
    for (let i = 0; i < pos.count; i += 3) {
      const dot =
        ((nrm.getX(i) + nrm.getX(i + 1) + nrm.getX(i + 2)) / 3) * axisZ.x +
        ((nrm.getY(i) + nrm.getY(i + 1) + nrm.getY(i + 2)) / 3) * axisZ.y +
        ((nrm.getZ(i) + nrm.getZ(i + 1) + nrm.getZ(i + 2)) / 3) * axisZ.z;
      // 单向：只留面朝原点的面（远侧面 = 穿透镜像，切向面 = 拉丝）
      if (dot > -DECAL_PROJECTION_NORMAL_CUTOFF) continue;
      for (let k = 0; k < 3; k += 1) {
        positions.push(pos.getX(i + k), pos.getY(i + k), pos.getZ(i + k));
        normals.push(nrm.getX(i + k), nrm.getY(i + k), nrm.getZ(i + k));
        uvs.push(uv.getX(i + k), uv.getY(i + k));
      }
    }
    if (!positions.length) return null;
    const filtered = new THREE.BufferGeometry();
    filtered.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(positions, 3),
    );
    filtered.setAttribute(
      "normal",
      new THREE.Float32BufferAttribute(normals, 3),
    );
    filtered.setAttribute("uv", new THREE.Float32BufferAttribute(uvs, 2));
    return filtered;
  }

  function buildDecalMaterial(
    THREE: typeof ThreeNamespace,
    dto: DecalUnitTexture,
    map: ThreeNamespace.Texture,
    opts: {
      halfDepth?: number;
      boxHalfX?: number;
      boxHalfY?: number;
      decalData: [number, number] | null;
      env: SunEnvRefs;
    },
  ): ThreeNamespace.Material {
    const base = {
      map,
      side: THREE.DoubleSide,
      // raw RGBA 直采（引擎 decal PS：`Current.color = tex2D(s0, uv)`）；
      // 纹理 alpha 是柔和衰减/掩码：sign 连续混合（光晕）、graffiti 阈值
      // 裁切（锐利边缘），见下方 switch。
      transparent: true,
      depthWrite: false,
      // 投影贴花与墙面共面，必须靠 polygonOffset 压过 z-fighting
      polygonOffset: true,
      polygonOffsetFactor: -4,
      polygonOffsetUnits: -4,
    };
    // 破洞族已由引擎材质链（createEngineDecalMaterial "hole"）接管，
    // buildDecalMaterial 不再处理 hole（手写回退路径 2026-10-05 退役）。
    switch (DECAL_MATERIAL_VARIANTS[(dto.materialInstance ?? 0) >>> 0]) {
      case "sign": {
        // decalNeonBrighten 逐字：`color.rgb *= shColorDiff + shColorSpec + spec`
        // ——招牌**响应场景光**（太阳/天空/lot 霓虹点灯，Lambert 即该响应的
        // PE 等价物：key 光随 env 昼夜驱动、夜间被 lot 真实点灯点亮）；
        // ×2 = decalFloatQuadNoClip 的增亮，过 hejl tonemap 保持亮度。
        const material = new THREE.MeshLambertMaterial({
          ...base,
          color: new THREE.Color(2, 2, 2),
        });
        applyEngineBlend(material, dto);
        return material;
      }
      case "graffiti": {
        // 涂鸦 = **连续 alpha 混合 + 受光**（引擎 decal PS 逐字：直采
        // raster + 标准 alpha 混合；raw alpha 字母 0.7-0.86 = 喷漆半透明、
        // 软边 = 抗锯齿，正是游戏观感）。此前 alphaTest 裁切会把 soft-alpha
        // 内容（涂鸦内部的房间/色块）整体裁掉——用户对拍"涂鸦不可辨认/
        // 破洞无内景"的根因（2026-09-30）。受光（MeshStandard）= 引擎
        // G-buffer 链的等价物（贴花写入 albedo 后统一光照）。
        const material = new THREE.MeshStandardMaterial({
          ...base,
          transparent: true,
          roughness: 1,
          metalness: 0,
        });
        applyEngineBlend(material, dto);
        return material;
      }
      default:
        // 未识别材质兜底同涂鸦口径（直采 + alpha 混合 + 受光）。
        return new THREE.MeshStandardMaterial({
          ...base,
          transparent: true,
          roughness: 1,
          metalness: 0,
        });
    }
  }

  /** 取贴花在 lot 局部的变换矩阵（无变换时为 None）。 */
  function applyDecalTransform(
    THREE: typeof ThreeNamespace,
    unit: DecalUnit,
    object: ThreeNamespace.Object3D,
  ) {
    if (!unit.transform) return;
    const matrix = unitMatrix(THREE, unit.transform);
    matrix.decompose(object.position, object.quaternion, object.scale);
  }

  /**
   * 精细模式贴花：所有族统一走引擎体积盒语义的 DecalGeometry 盒裁剪投影
   * （建筑三角形 → 贴面网格，曲面/阶梯立面自然贴合；破洞族 2026-10-05
   * 并入此路径，体积盒 BackSide 方案退役）。盒内无建筑面：破洞族按
   * decalClip 语义不渲染，其余族 = 引擎浮空分支（decalFloatQuad）：
   * 数据位姿的浮空 quad（holo 广告/远抛实例）。
   *
   * 返回的顶层对象是**位于贴花原点的 Group**，使 TransformControls 挂在原点、
   * `unitObjects` 选中与 `userData.unitId` 注册照旧；投影几何子节点用
   * 逆矩阵抵消父变换，因此几何本身保持 lot 局部坐标。
   */
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
    const aspect =
      texture.aspectRatio && texture.aspectRatio > 0 ? texture.aspectRatio : 1;
    const decoded = await getDecalTexture(THREE, texture);
    if (!decoded) return null;
    const frame = decalFrame(THREE, unit, aspect);
    if (!frame) {
      console.warn(
        "[decal] 无有效 transform/scale，无法构建投影帧：",
        unitId(unit),
        "transform=",
        unit.transform
          ? `matrixLen=${unit.transform.matrix?.length ?? 0}`
          : "null",
        "scale=",
        unit.scale,
      );
      return null;
    }
    // 地面贴花特例已删除（2026-09-30 用户裁定）：引擎 decal 18 族源码无任何
    // 向地面/地形投影的设计（grep ground/terrain 仅命中同容器的地形着色器
    // 常量）——decal = 变换矩阵摆位的四边形，朝向完全由 transform 决定。
    // 地面平行贴花照常走下方统一投影路径（盒体可命中建筑檐口/基座等结构，
    // 未命中则回退浮空 quad，姿态仍由 transform 给出）。
    const group = new THREE.Group();
    applyDecalTransform(THREE, unit, group);

    // 路由（2026-10-01 引擎对齐重分析，docs/re/decal-engine-alignment.md）：
    // Ghidra SC_cVolumeDecalManager FUN_006fdce0 证明 decal 体积盒由变换数据
    // 直接构造，**无射线/无距离判定**——早前的 ≤10m 投影路由是发明物，删除。
    // 现行分派：所有族统一走 DecalGeometry 投影（破洞族 2026-10-05 起并入，
    // 体积盒 BackSide 路径退役——盒内壁不随建筑曲面走 = 破洞不贴合/漂浮根因；
    // 引擎投影/浮空由 shader 选择固化，破洞 shader decalInteriorMap 内含
    // decalClip 体积裁剪，几何上就是投影贴面网格）。
    if (frame) {
      // 引擎体积盒的 CPU 同构（2026-10-05 四轮定稿，casino 探针实锤）：
      // **盒深 = unit.depth × scale**——lot 0x457EA9DB 全部 7 个 decal：
      // 两招牌 scale 13.27/5.63 而 depth×scale 恒 = 1.327m，五涂鸦恒
      // ≈17.95m → depth 是按 scale 归一化的盒进深。盒 = xy 居中于原点、
      // z∈[0, depth×scale] 沿 **+axisZ**（63/63 足迹射线全部命中 +局部Z；
      // 引擎 decal 面向约定 -z 朝街 ⇒ 墙恒在 +axisZ），与 RL
      // `SC::cDecalData::SpawnDecalInstance` 的 model→texture z∈[0,depth]
      // 映射同一语义。招牌盒仅 1.33m 深 ⇒ 背墙/对侧玻璃天然在盒外
      // （四轮图1 回归根因 = 此前 ~2.5×scale 深盒吞背墙 + "最近命中侧"
      // 把原点偏玻璃侧的招牌定错侧——两者皆删，不再选侧）。
      const halfScale = frame.sizeY / 2;
      const dataDepth =
        unit.depth !== null && Number.isFinite(unit.depth) && unit.depth > 0
          ? unit.depth * halfScale
          : null;
      // 单向 +axisZ 射线判定盒内是否有建筑面（far = 数据盒深；无 depth
      // 数据时退回旧 2.5×scale 探测半径）。
      const raycaster = new THREE.Raycaster(
        frame.origin,
        frame.axisZ,
        0,
        dataDepth ?? halfScale * 2.5,
      );
      const wallHit = raycaster.intersectObjects(proxies, false)[0];
      const { DecalGeometry } = await import(
        "three/examples/jsm/geometries/DecalGeometry.js"
      );
      if (ctx.isStale()) return null;
      // 族路由（2026-10-05 破洞并入投影路径）：破洞（decalInteriorMap）
      // 直接路由 hole 链（投影几何 + 内景双 UV + 受光步，与原版片段链
      // [379]/[380] 逐字对齐）；量化族字典（招牌 + 涂鸦）内再按
      // materialData[1] 分流——该分量 = 引擎 decalMaterialInfo.y =
      // **animSpeed 跑马灯速度**（docs/re/decal-engine-alignment.md §十五，
      // 早前"疑似自发光参数"的猜测已被 decalLightBackground 源码取代）：
      //   > 0 → SDF 霓虹管动画链（自发光 + 跑马灯，纹理四通道 = 四路 SDF
      //         距离场；实证：高塔 STORE 1.0 / DIRTY FACTORY 0.06）；
      //   = 0 → 量化合成静态链（casino 招牌 / 涂鸦恒 0）；
      // 其余（焦痕/烧灼/未知）→ clip 直采链。
      const animSpeed = unit.materialData?.[1] ?? 0;
      const family: EngineFamily = DECAL_HOLE_MATERIALS.has(
        (texture.materialInstance ?? 0) >>> 0,
      )
        ? "hole"
        : DECAL_QUANT_MATERIALS.has((texture.materialInstance ?? 0) >>> 0)
          ? animSpeed > 0
            ? "sdf"
            : "sign"
          : "clip";
      if (family === "sdf") neonAnimated = true;
      // 引擎浮空分支（decalFloatQuad，holo 广告/远抛实例）：在**数据
      // 位置**画浮空 quad，姿态由 transform 给出——不是"不渲染"（四轮
      // 图3/4 的 holo 被错误贴上墙 = 此前缺失浮空分支、万物皆投影的
      // 回归根因）。组已带 unit transform，quad 在组局部恒等姿态即
      // 数据位姿；材质 DoubleSide 双面可见。
      const addFloatQuad = () => {
        const floatMaterial = ENGINE_SHADER_MATERIALS
          ? createEngineDecalMaterial(THREE, family, {
              map: decoded,
              layerColors: texture.colors,
              decalData: props.decalLight ?? null,
              worldDirection: frame.axisZ,
              env,
              materialInfo: unit.materialData ?? undefined,
              powered: props.powered,
              nus: [frame.sizeX, frame.sizeY, frame.sizeY],
              graffiti:
                ((texture.materialInstance ?? 0) >>> 0) === 0xe5390a98,
            })
          : buildDecalMaterial(THREE, texture, decoded, {
              env,
              halfDepth: decalHalfThickness(unit.depth),
              boxHalfX: frame.sizeX / 2,
              boxHalfY: frame.sizeY / 2,
              decalData: props.decalLight ?? null,
            });
        const quadGeometry = new THREE.PlaneGeometry(frame.sizeX, frame.sizeY);
        const quadUv = quadGeometry.attributes.uv;
        for (let i = 0; i < quadUv.count; i += 1) quadUv.setX(i, 1 - quadUv.getX(i));
        quadUv.needsUpdate = true;
        group.add(new THREE.Mesh(quadGeometry, floatMaterial));
        decalStats.fallback += 1;
      };
      if (!wallHit) {
        // 破洞族无浮空分支：引擎 decalInteriorMap 链内含 decalClip 体积
        // 裁剪——体积盒内无场景深度（未触及建筑面）时整贴花被 clip kill，
        // 即"根本不渲染"，不是回退浮空（§12.4/§12.7）。
        if (family === "hole") return group;
        addFloatQuad();
        return group;
      }
      // 浮空族数据判据（2026-10-05 五轮定谳，docs/re/decal-engine-alignment
      // §十四）：**sign 字典且 materialData[1] ≥ 0.9 → 引擎 FloatQuad 族**
      // （holo/竖幅灯牌），跳过投影直接画浮空 quad。实证对拍：
      // casino 墙招牌 md[1]=0（投影 ✓）、高塔 0x9401CB7A 竖幅 STORE
      // md[1]=0.95/1.0（用户确认浮空 ✓）、DIRTY FACTORY md[1]=0.06
      // （投影 ✓）、涂鸦恒 0（投影 ✓）。md[1] 语义已由 §十五定谳 =
      // animSpeed（引擎 decalMaterialInfo.y）——高速动画招牌恰是引擎的
      // 浮空灯箱族，与 FloatQuad 族选择同源。注意六轮起 md[1]>0 的招牌
      // 已分流到 sdf 族，故判据须同时覆盖 sign/sdf（否则高塔竖幅会被
      // 错误投影——路由升级引入的回归点）。几何判据（离墙距离/盒深）
      // 已被涂鸦 10m 离墙反例证伪。安全网：贴墙摆放的 sign 浮空 quad 与
      // 投影观感近乎一致（原点即在墙面），误判代价低。
      if (
        (family === "sign" || family === "sdf") &&
        animSpeed >= DECAL_FLOAT_EMISSIVE_CUTOFF
      ) {
        addFloatQuad();
        return group;
      }
      // 单侧盒投影：z 全深 = 数据盒深（无 depth 数据 = 命中距离 + 2m
      // 檐口/退台余量），前缘在原点、沿 +axisZ 延伸。
      const depthZ = dataDepth ?? wallHit.distance + 2.0;
      const projector = {
        position: frame.origin
          .clone()
          .addScaledVector(frame.axisZ, depthZ / 2),
        orientation: new THREE.Euler().setFromRotationMatrix(frame.matrix),
        size: new THREE.Vector3(frame.sizeX, frame.sizeY, depthZ),
      };
      const boxRadius =
        Math.hypot(projector.size.x, projector.size.y, projector.size.z) / 2;
      const geometries: ThreeNamespace.BufferGeometry[] = [];
      for (const proxy of proxies) {
        // 包围球粗筛：盒外建筑网格跳过逐三角形裁剪
        if (!proxy.geometry.boundingSphere) {
          proxy.geometry.computeBoundingSphere();
        }
        const sphere = proxy.geometry.boundingSphere;
        if (
          sphere &&
          sphere.center.distanceTo(projector.position) >
            sphere.radius + boxRadius
        ) {
          continue;
        }
        const clipped = new DecalGeometry(
          proxy,
          projector.position,
          projector.orientation,
          projector.size,
        );
        if ((clipped.attributes.position?.count ?? 0) === 0) continue;
        // 破洞族不按法线过滤：引擎延迟投影对体积盒内**所有**表面逐像素
        // 落贴花——破洞后露出的楼板/内墙/远侧内壁正是游戏"内部结构"的
        // 来源（N·axisZ≤−0.5 过滤会把水平楼板全部滤掉 = 平斑）。FrontSide
        // 背面剔除 + 建筑实体不透明深度遮挡已防外侧面穿透。
        const filtered =
          family === "hole"
            ? clipped
            : filterDecalProjectionByNormal(THREE, clipped, frame.axisZ);
        if (filtered) geometries.push(filtered);
      }
      // 射线命中但法线过滤后无任何可投面（墙面与投影轴近平行的极端
      // 摆放）→ 破洞同样不渲染（decalClip 语义）；其余族走浮空分支：
      // 宁可画在数据位姿也不凭空消失。
      if (!geometries.length) {
        if (family === "hole") return group;
        addFloatQuad();
        return group;
      }
      // U 镜像（与 quad 路径同口径：引擎 uv = texpos × -0.5 + 0.5，文字正读）
      for (const geometry of geometries) {
        const uv = geometry.attributes.uv;
        for (let i = 0; i < uv.count; i += 1) uv.setX(i, 1 - uv.getX(i));
        uv.needsUpdate = true;
      }
      const engineMaterial = ENGINE_SHADER_MATERIALS
        ? createEngineDecalMaterial(THREE, family, {
            map: decoded,
            layerColors: texture.colors,
            decalData: props.decalLight ?? null,
            worldDirection: frame.axisZ,
            env,
            materialInfo: unit.materialData ?? undefined,
            powered: props.powered,
            nus: [frame.sizeX, frame.sizeY, frame.sizeY],
            // 涂鸦（0xE5390A98）：喷漆连续厚度 alpha + 无灯箱增益/夜间
            // 豁免（rt0 编译状态定谳：标准 alpha 混合，无自发光项）
            graffiti: ((texture.materialInstance ?? 0) >>> 0) === 0xe5390a98,
            // 投影网格只画正面：薄板/单面墙从背后看时，decal 三角形是
            // 背面 → 剔除，杜绝"隔着建筑看到镜像字"（五轮图1；法线过滤
            // 已保证留下的面都朝原点，正面即被投面）。
            side: THREE.FrontSide,
            // 破洞族：盒参数供 VS 计算 tfp.z 真实进深（立面 −1 全尺寸
            // 内景、破洞后深部结构 +1 中心收缩 = 游戏"内部结构"机制）。
            // 盒前缘 = 变换原点、沿 +axisZ 延伸 depthZ（与投影盒同口径）。
            // halfXY/depthM/invRot 供 PS 视线视差（holeParallaxUv）。
            holeBox:
              family === "hole"
                ? {
                    origin: frame.origin,
                    axisZ: frame.axisZ,
                    invDepth: 1 / Math.max(depthZ, 0.01),
                    halfXY: [frame.sizeX / 2, frame.sizeY / 2],
                    depthM: depthZ,
                    // lot→贴花系旋转 = 基矩阵转置（行主序 set：行 = 各基
                    // 向量，正交基下转置即逆）
                    invRot: new THREE.Matrix3().set(
                      frame.axisX.x, frame.axisX.y, frame.axisX.z,
                      frame.axisY.x, frame.axisY.y, frame.axisY.z,
                      frame.axisZ.x, frame.axisZ.y, frame.axisZ.z,
                    ),
                  }
                : undefined,
          })
        : buildDecalMaterial(THREE, texture, decoded, {
            env,
            halfDepth: decalHalfThickness(unit.depth),
            boxHalfX: frame.sizeX / 2,
            boxHalfY: frame.sizeY / 2,
            decalData: props.decalLight ?? null,
          });
      // 投影几何 = lot 局部坐标（代理网格矩阵为恒等）；容器带组局部逆
      // 变换抵消组上的 unit transform（与旧 quad 的位置逆变换同机制）。
      const container = new THREE.Group();
      container.matrix.copy(frame.matrix).invert();
      container.matrixAutoUpdate = false;
      for (const geometry of geometries) {
        container.add(new THREE.Mesh(geometry, engineMaterial));
      }
      group.add(container);
      decalStats.projected += 1;
      // 破洞视线视差：每帧把相机位置变换到 mesh 局部（= lot 局部）空间写
      // 入 uHoleCamLot（container 的 matrixWorld = lot 世界矩阵 × 组链上
      // 相消的 unit 变换；onBeforeRender 在 updateMatrixWorld 之后触发）。
      if (family === "hole" && ENGINE_SHADER_MATERIALS) {
        const shaderMat = engineMaterial as ThreeNamespace.ShaderMaterial;
        container.onBeforeRender = (_renderer, _scene, camera) => {
          const camLot = container.worldToLocal(camera.position.clone());
          (shaderMat.uniforms.uHoleCamLot.value as ThreeNamespace.Vector3).copy(camLot);
        };
      }
      // decalInteriorMap 光 pass 近似（保留自体积盒时代）：lot 带光参数时，
      // 沿投影轴向墙面投暖色 cookie 光（引擎用贴花贴图作光 cookie、alpha
      // 作衰减）。上限 8 盏防多破洞 lot 光源洪峰。
      if (family === "hole" && props.decalLight && holeLightCount < HOLE_LIGHT_MAX) {
        const [scaleFactor, radiusFactor] = props.decalLight;
        const spot = new THREE.SpotLight(
          0xffdca0,
          (scaleFactor * 16 + 1) * 3,
          radiusFactor * 8,
          0.9,
          0.6,
          1,
        );
        spot.map = decoded;
        spot.position.set(0, 0, -0.5);
        spot.target.position.set(0, 0, 1);
        group.add(spot, spot.target);
        holeLightCount += 1;
      }
      return group;
    }
    return group;
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
  const decalUnits: DecalUnit[] = [];
  let unitBudget = 0;
  for (const unit of units) {
    if (unit.kind === "decal") {
      decalUnits.push(unit);
      continue;
    }
    // 每 8 个 unit 让出一帧：单元渲染器虽是 async，缓存命中时同步 resolve
    // → 整段循环实际单任务执行；树模板命中可达数十 unit，不切片则循环
    // 期间输入全程卡死。循环内已有逐 unit isStale 检查。
    if (++unitBudget >= 8) {
      unitBudget = 0;
      await yieldToBrowser();
      if (ctx.isStale()) return;
    }
    // 精细模式：光源用真实 three.js 光源；道具/spawner 走资产渲染分发
    //（tree/prop/spawner 独立渲染器，互不回归——2026-10-06 拆分）。
    // 失败统一退标记锥。
    const object =
      (props.renderMode === "refined" && unit.kind === "light"
        ? buildRealLightUnit(THREE, unit)
        : null) ??
      (await renderRefinedAssetUnit(assetCtx, unit)) ??
      buildUnitObject(THREE, unit);
    if (!object) continue;
    // 统一在此登记：精细模式的道具/spawner 不走 buildUnitObject 时其
    // userData 由此补齐（此前精细模式光源因此点不中）。
    object.userData.unitId = unitId(unit);
    object.userData.unitKind = unit.kind;
    instance.group(kindGroup(unit.kind)).add(object);
    ctx.unitObjects.set(unitId(unit), object);
    if (ctx.isStale()) return;
  }

  // ---- 汇合点：三线全部落地（或已因过期自行退出）----
  const ground = await groundStage;
  await modelStage;
  await decalPrefetch;
  if (ctx.isStale()) return;

  // Lot 三维尺度标注（用户需求 2026-10-06）：长/宽取地面矩形两邻边，
  // 高取建筑 bbox 实际高度（世界系 Y 跨度；世界仅旋转，跨度=游戏系 Z）。
  if (props.lotSize && ground) {
    instance.scene.updateMatrixWorld(true);
    const modelBox = new THREE.Box3().setFromObject(instance.group("model"));
    const lotHeight = modelBox.isEmpty()
      ? 0
      : Math.max(0, modelBox.max.y - modelBox.min.y);
    const dims = buildLotDimensions(THREE, props.lotSize, lotHeight);
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
  holeLightCount = 0;
  neonAnimated = false;
  // 吸附射线目标：建筑网格代理（quad 摆放用）
  const decalProxies =
    props.renderMode === "refined" && buildingMeshes.length
      ? buildingMeshes.map((mesh) => new THREE.Mesh(mesh.geometry))
      : [];
  for (const unit of decalUnits) {
    const decalTexture = decalTextureByKey.get(
      `${unit.category}:${unit.index}`,
    );
    if (props.renderMode === "refined" && !decalTexture) {
      console.warn(
        `[decal] 配对失败 cat${unit.category}:idx${unit.index}，已注册键：`,
        [...decalTextureByKey.keys()],
      );
    }
    let object: ThreeNamespace.Object3D | null;
    if (props.renderMode === "refined" && decalTexture) {
      // 贴图解码失败（无 png）→ 退回 gizmo，保证仍可见可选
      object =
        (await buildDecalObject(
          THREE,
          unit,
          decalTexture,
          buildingMeshes,
          decalProxies,
        )) ?? buildUnitObject(THREE, unit);
    } else {
      object = buildUnitObject(THREE, unit);
    }
    if (!object) continue;
    object.userData.unitId = unitId(unit);
    object.userData.unitKind = unit.kind;
    instance.group(kindGroup(unit.kind)).add(object);
    ctx.unitObjects.set(unitId(unit), object);
    if (ctx.isStale()) return;
  }
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
}

/** 日/夜亮度：仅缩放环境四灯；地块真实光源保持常亮（夜间灯依然亮）。 */
function applyBrightness() {
  // 5d：夜间环境光随白昼因子压暗（亮度滑杆仍是用户侧总倍率）
  viewport.viewer.value?.setEnvironmentBrightness(
    brightness.value * (0.22 + 0.78 * dayFactor(timeOfDay())),
  );
}

defineExpose({
  captureRender: (options?: { includeDecals?: boolean }) =>
    viewport.captureRender(options),
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
  () => props.grouping,
  (grouping) => {
    if (!tryIncrementalGrouping(grouping)) scheduleRebuild("grouping");
  },
);
watch(
  () => props.groupVisibility,
  () => viewport.applyGroupVisibility(props.groupVisibility),
  { deep: true },
);
// 异步资产通道到位 → **部分刷新**受影响族（prop/spawner 原位换对象），
// 不再整场重建（2026-10-07：此前四通道各触发一次全量重建，建筑/地面/
// 贴花全部白跑——遥测 2154 重建/682 加载 ≈ 3.2× 的主要成分）。初始重建
// 未就绪时 unitObjects 为空，刷新自然 no-op，由重建本身消费新载荷。
watch(
  () => [
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
      >{{ dimAxisLabel[dim.axis] }} {{ dim.text }}</span>
      <span
        :ref="(el) => (selectTagEl = el as HTMLElement | null)"
        class="pick-label pick-tag"
        aria-hidden="true"
      >
        <span class="pick-tag-text" />
        <button
          v-if="props.editEnabled"
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
      v-if="modelState === 'loading'"
      class="viewport-overlay viewport-status"
    >
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
      v-if="props.editEnabled"
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
