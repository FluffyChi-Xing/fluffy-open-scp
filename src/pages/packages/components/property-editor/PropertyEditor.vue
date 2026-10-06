<script setup lang="ts">
import { computed, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FAlert from "@/components/ui/FAlert.vue";
import FTooltip from "@/components/ui/FTooltip.vue";
import FCheckbox from "@/components/ui/FCheckbox.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FSheet from "@/components/ui/FSheet.vue";
import FCode from "@/components/ui/FCode.vue";
import type {
  LotModelPayload,
  LotUnitDto,
  Tgi,
} from "@/api/tauri";
import {
  parseLotModelContainer,
} from "@/lib/three-gltf";
import { createDataSource } from "@/api/data-source";
import PropertyEditorOutliner from "./PropertyEditorOutliner.vue";
import PropertyEditorViewport, {
  type EditorTool,
} from "./PropertyEditorViewport.vue";
import PropertyEditorInspector from "./PropertyEditorInspector.vue";
import PropertyEditorStatusBar from "./PropertyEditorStatusBar.vue";
import { unitId } from "./unitGizmos";
import { buildSchemaDoc } from "./peSchemaDoc";
import { usePropertyEditorSession } from "./usePropertyEditorSession";
import { useEditorHotkeys } from "./useEditorHotkeys";
import { exportLotModel } from "@/composables/useModelExport";
import { useEventListener } from "@vueuse/core";
import { useI18n } from "vue-i18n";
import { command } from "@/api/tauri";
import { tauriApi } from "@/api";

const props = defineProps<{ packageId: number; tgi: Tgi }>();
const { t } = useI18n();
const open = defineModel<boolean>("open", { default: false });
const {
  session,
  loading,
  loadError,
  modelPayload,
  propModels,
  propTreeIds,
  referencedModels,
  treeAtlasPng,
  treeModelPayloads,
  releasePropPackages,
  modelLods,
  activeLod,
  switchLod,
  modelState,
  selectedId,
  grouping,
  flatUnits,
  lotSize,
  lotTilePeriod,
  lotPlacement,
  lotColors,
  lotColorsAuthored,
  lotBorderColors,
  lotBorderWidths,
  lotBorderPatternIndices,
  lotBaseTile,
  lotOverlayBoxOffset,
  lotModelBBoxCenter,
  lotMaskPng,
  lotMaskRawRgba,
  lotAlbedoPng,
  lotSurfacePng,
  lotNormalAtlasPng,
  decalLight,
  selectedUnit,
  edit,
  hiddenUnits,
  groupVisibility,
  load,
  toggleGroup,
  toggleUnit,
  resetViewState,
} = usePropertyEditorSession(props.packageId, props.tgi);

/** 编辑工具（PE-重构-3）：select = 仅拾取；translate/rotate/scale 挂手柄。 */
const tool = ref<EditorTool>("select");
function setTool(next: EditorTool) {
  tool.value = next;
}

/** 手柄拖拽中的实时变换（坐标面板即时显示；id 为 null = 拖拽结束）。 */
const liveTransform = ref<{
  id: string;
  position: [number, number, number];
} | null>(null);
function onLiveTransform(
  id: string | null,
  value: { position: [number, number, number] } | null,
) {
  liveTransform.value = id && value ? { id, position: value.position } : null;
}

/** 手柄/坐标面板提交变换 → 本地可撤销命令。 */
function commitTransform(id: string, matrix: number[]) {
  edit.setUnitTransform(id, matrix);
}
/** 元数据面板提交字段 patch → 本地可撤销命令。 */
function commitFields(id: string, patch: Record<string, unknown>) {
  edit.setUnitFields(id, patch);
}

/**
 * 选中 prop 是否可编辑「缩放倍率」：仅真实模型渲染分支消费 DTO scale
 * （树模型 = propTreeIds；prop/放置直挂模型 = 有 LOTM 载荷）。标记锥
 * 兜底的 prop 编辑它无观感意义，不显示。
 */
const selectedScaleEditable = computed(() => {
  const unit = selectedUnit.value;
  if (!unit || unit.kind !== "prop") return false;
  const rid = unit.resourceId;
  if (typeof rid !== "number") return false;
  return (
    propTreeIds.value.has(rid) ||
    propModels.value.has(rid) ||
    addedModelPayloads.value.has(rid)
  );
});

/**
 * 显式保存：本地编辑导出为 JSON 补丁文件。当前编辑仅存在于内存，
 * 不触碰任何游戏 package；真正的 DBPF overlay 写回在资产-1（RW4
 * 写回器）落地后接入，届时同样以此按钮为唯一入口。
 */
// ---- 编辑模式（锁定/解锁）----
// 锁定 = 只读视图（原"只读视图"pill 的交互化）；解锁后：视口工具栏显示、
// rail 物料入口可用、组件可拖入画布、name-tag 旁出现删除按钮。
const editEnabled = ref(false);
function toggleEditEnabled() {
  editEnabled.value = !editEnabled.value;
}

// ---- 组件库（物料）sheet：从已打开包的命名 RW4 模型目录拖入放置 ----
const materialsOpen = ref(false);
const materialsBusy = ref(false);
interface ModelCatalogEntry {
  packageId: number;
  instance: number;
  name: string;
  size: number;
}
const modelCatalog = ref<ModelCatalogEntry[]>([]);
const materialsTab = ref<"props" | "spawners" | "effects" | "paths" | "lights">(
  "props",
);
const materialsSearch = ref("");
/** 折叠面板展开状态（按组名；缺省全展开）。 */
const collapsedGroups = ref(new Set<string>());
function openMaterialsPanel() {
  materialsOpen.value = true;
  if (modelCatalog.value.length || materialsBusy.value) return;
  materialsBusy.value = true;
  tauriApi.packages
    .listModelCatalog()
    .then((entries) => {
      modelCatalog.value = entries;
    })
    .catch(() => {
      modelCatalog.value = [];
    })
    .finally(() => {
      materialsBusy.value = false;
    });
}
/** props 目录按名称首段（分隔符前）分组建目（可折叠）。 */
const catalogGroups = computed<{ name: string; entries: ModelCatalogEntry[] }[]>(
  () => {
    const keyword = materialsSearch.value.trim().toLowerCase();
    // 过滤器（用户口径）：只展示当前 property 引用的组件——这些模型的
    // 渲染路径已被本资产验证，拖入必然可用（随机目录模型可能无法渲染）
    const referencedSet = new Set(referencedModels.value.map((r) => r.instance));
    const packageOf = new Map(
      referencedModels.value.map((r) => [r.instance, r.packageId]),
    );
    const matched = modelCatalog.value.filter(
      (entry) =>
        referencedSet.has(entry.instance) &&
        (keyword ? entry.name.toLowerCase().includes(keyword) : true),
    );
    // 引用了但目录缺席的模型（纯 hex 名被目录噪声过滤排除，如树部件）
    // → 兜底补入：包 id 取引用条目（readLotModelMeshes 可按需加载）
    for (const refEntry of referencedModels.value) {
      if (matched.some((entry) => entry.instance === refEntry.instance)) continue;
      const hexName = t("package.modelFallbackName", {
        hex: `0x${refEntry.instance.toString(16).toUpperCase()}`,
      });
      if (keyword && !hexName.toLowerCase().includes(keyword)) continue;
      matched.push({
        packageId: refEntry.packageId,
        instance: refEntry.instance,
        name: hexName,
        size: 0,
      });
    }
    const groups = new Map<string, ModelCatalogEntry[]>();
    for (const entry of matched) {
      const group = entry.name.split(/[_\s-]/)[0] || entry.name;
      if (!groups.has(group)) groups.set(group, []);
      groups.get(group)!.push(entry);
    }
    return [...groups.entries()]
      .map(([name, list]) => ({ name, entries: list }))
      .sort((a, b) => a.name.localeCompare(b.name));
  },
);
function toggleGroupCollapse(name: string) {
  const next = new Set(collapsedGroups.value);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  collapsedGroups.value = next;
}



// ---- 放置/删除：unitEditLayer 资产编辑 + 视口回调 ----
interface PlacePayload {
  kind: "light" | "prop" | "spawner" | "effect" | "pathPoint";
  /** light 预设。 */
  lightType?: "Point" | "Spot" | "Line";
  /** prop 直挂模型。 */
  packageId?: number;
  tgi?: Tgi;
  name?: string;
}
const addedModelPayloads = ref(new Map<number, LotModelPayload>());
/** 放置后待选中的 unit id：grouping 重建（revision 前进）后由 Viewport
 * emit select 落地；取消选中时由壳清空避免重复选中。 */
const pendingSelectId = ref<string | null>(null);
watch(selectedId, (value) => {
  if (!value) pendingSelectId.value = null;
});
const sourceForPlacement = createDataSource();

/** 放置 prop（树部件家族）的默认缩放：0.2 × 原生 ~51m ≈ 10m 树高
 * （对拍消防局 lot 树道具 scale 分布的两级中值），缩放手柄可调。 */
const PLACED_PROP_SCALE = 0.2;
function nextIndexOf(kind: string): number {
  const units = session.value?.units ?? [];
  let max = -1;
  for (const unit of units) {
    if (unit.kind !== kind) continue;
    if (unit.index > max) max = unit.index;
  }
  for (const added of edit.addedUnits) {
    if (added.kind !== kind) continue;
    if (added.index > max) max = added.index;
  }
  return max + 1;
}
// 指针级放置拖拽（@vueuse/core useEventListener）：pointerdown 拾起面板
// 条目 → window 跟踪指针 → pointerup 落点放置。不走 HTML5 DnD——FSheet
// 全屏遮罩会拦截原生拖放并给出禁止光标（真机勘误 2026-10-06）。
const placementPayload = ref<PlacePayload | null>(null);
useEventListener(window, "pointerup", (event: PointerEvent) => {
  const payload = placementPayload.value;
  if (!payload) return;
  placementPayload.value = null;
  document.body.style.cursor = "";
  const position = viewportRef.value?.groundPointAt(
    event.clientX,
    event.clientY,
  );
  if (!position) return; // 落点不在地面平面（视线朝天）→ 取消
  void onPlaceUnit(payload, position);
});
function beginPlacementDrag(event: PointerEvent, payload: PlacePayload) {
  if (!editEnabled.value) return;
  event.preventDefault();
  placementPayload.value = payload;
  document.body.style.cursor = "copy";
}
function onPlaceUnit(payload: PlacePayload, position: [number, number, number]) {
  if (!editEnabled.value) return;
  // 基向量 = 同类既有单元的中位缩放（引擎按单元数据缩放聚合模型；
  // 数据无缩放时退化为 1）
  // 放置 prop（树部件家族）默认缩放 0.2：×原生 ~51m ≈ 10m 树高
  // （对拍消防局 lot 树道具 scale 分布的两级中值），缩放手柄可调。
  const placedScale = payload.kind === "prop" ? PLACED_PROP_SCALE : 1;
  const transform = {
    matrix: [
      placedScale, 0, 0, 0, placedScale, 0, 0, 0, placedScale, 0, 0,
      position[0], position[1], position[2],
    ],
  };
  let unit: LotUnitDto;
  if (payload.kind === "light") {
    unit = {
      kind: "light",
      index: nextIndexOf("light"),
      transform,
      lightType: payload.lightType ?? "Point",
      color: [1, 1, 1],
      outerRadius: 8,
      innerRadius: null,
      diffuse: null,
      length: null,
      cullDistance: "Far",
      isVolumetric: false,
      debugName: null,
      fields: [],
    } as LotUnitDto;
  } else if (payload.kind === "prop" && payload.tgi && payload.packageId != null) {
    unit = {
      kind: "prop",
      index: nextIndexOf("prop"),
      bin: 1,
      resourceId: payload.tgi.instance,
      transform,
      slot: null,
      scale: null,
      modelTgi: payload.tgi,
      modelPackageId: payload.packageId,
      fields: [],
    } as unknown as LotUnitDto;
  } else if (payload.kind === "spawner") {
    unit = {
      kind: "spawner",
      index: nextIndexOf("spawner"),
      transform,
      id: null,
      count: null,
      countRandom: null,
      agent: null,
      fields: [],
    } as LotUnitDto;
  } else if (payload.kind === "effect") {
    unit = {
      kind: "effect",
      index: nextIndexOf("effect"),
      transform,
      effectId: null,
      enabled: true,
      fields: [],
    } as LotUnitDto;
  } else {
    unit = {
      kind: "pathPoint",
      index: nextIndexOf("pathPoint"),
      point: position,
      tangent: null,
      pointIndex: null,
      fields: [],
    } as LotUnitDto;
  }
  edit.addUnit(unit);
  pendingSelectId.value = unitId(unit);
  // 直挂 prop：异步取 LOTM 载荷入缓存（渲染器从此读取）
  if (payload.kind === "prop" && payload.tgi && payload.packageId != null) {
    const instance = payload.tgi.instance;
    sourceForPlacement
      .readLotModelMeshes(payload.packageId, {
        typeId: 0x2f4e_681b,
        group: 0,
        instance,
      })
      .then((buffer) => {
        const next = new Map(addedModelPayloads.value);
        next.set(instance, parseLotModelContainer(buffer));
        addedModelPayloads.value = next;
      })
      .catch(() => {
        // 载荷失败：标记锥兜底（渲染分支自处理）
      });
  }
}
function onDeleteUnit(id: string) {
  edit.removeUnit(id);
  if (selectedId.value === id) selectedId.value = null;
}

// ---- Schema（低代码资产管线 v1，docs/design/lowcode-asset-pipeline.md）----
// 活体文档：编辑态（session + 编辑层覆盖合并后的 effectiveUnits）的前端
// JSON 投影，openscp.lot-asset/1 结构与后端 build_schema 同构——解锁编辑
// 后用户的一切增删改查都实时反映在这份 JSON 上（渲染/检查器/Schema
// sheet 三方同源），导出即所得。FCode 只读预览，导出写 .lot.json。
const schemaOpen = ref(false);
/** 手动刷新代数：活体 computed 本就随编辑联动，此钮是显式保险
 * （用户指令 2026-10-06：编辑后要能手动刷新 schema JSON）。 */
const schemaRevision = ref(0);
/** 变更对比开关：开 = FCode diff 视图（编辑基线 → 当前活体）。 */
const schemaDiff = ref(false);
const schemaJson = computed(() => {
  // 读 revision 进依赖：刷新按钮强制重算（其余依赖 reactive 自动联动）
  void schemaRevision.value;
  const current = session.value;
  if (!current) return "";
  // flatUnits = effectiveUnits（session 已合并 overrides/fieldOverrides/
  // 增删）——文档即渲染链路正在消费的生效值
  return JSON.stringify(
    buildSchemaDoc({
      assetName: current.assetName ?? null,
      tgi: props.tgi,
      modelLods: current.modelLods,
      lotSize: current.lotSize,
      lotTilePeriod: current.lotTilePeriod,
      lotPlacement: current.lotPlacement,
      lotBaseTile: current.lotBaseTile,
      lotColors: current.lotColors,
      lotColorsAuthored: current.lotColorsAuthored,
      lotBorderColors: current.lotBorderColors,
      lotBorderWidths: current.lotBorderWidths,
      lotBorderPatternIndices: current.lotBorderPatternIndices,
      lotOverlayBoxOffset: current.lotOverlayBoxOffset,
      lotModelBBoxCenter: current.lotModelBBoxCenter,
      units: flatUnits.value,
      hiddenUnitIds: [...hiddenUnits.value],
      groups: { ...groupVisibility },
    }),
    null,
    2,
  );
});
/** 编辑基线 = session 原始单元（无编辑层覆盖、无隐藏）——diff 的 before 侧。 */
const schemaBaseJson = computed(() => {
  const current = session.value;
  if (!current) return "";
  return JSON.stringify(
    buildSchemaDoc({
      assetName: current.assetName ?? null,
      tgi: props.tgi,
      modelLods: current.modelLods,
      lotSize: current.lotSize,
      lotTilePeriod: current.lotTilePeriod,
      lotPlacement: current.lotPlacement,
      lotBaseTile: current.lotBaseTile,
      lotColors: current.lotColors,
      lotColorsAuthored: current.lotColorsAuthored,
      lotBorderColors: current.lotBorderColors,
      lotBorderWidths: current.lotBorderWidths,
      lotBorderPatternIndices: current.lotBorderPatternIndices,
      lotOverlayBoxOffset: current.lotOverlayBoxOffset,
      lotModelBBoxCenter: current.lotModelBBoxCenter,
      units: current.units as LotUnitDto[],
      hiddenUnitIds: [],
      groups: {},
    }),
    null,
    2,
  );
});
async function exportSchema() {
  if (!schemaJson.value) return;
  const binary = JSON.stringify(JSON.parse(schemaJson.value), null, 2);
  let payload = "";
  for (const byte of new TextEncoder().encode(binary))
    payload += String.fromCharCode(byte);
  const path = await tauriApi.packages.saveFile(
    `${session.value?.assetName ?? "lot"}.lot.json`,
    "json",
  );
  if (!path) return;
  await command("write_export_file", {
    request: { path, dataBase64: btoa(payload) },
  });
}
const saveEditsBusy = ref(false);
async function saveLocalEdits() {
  if (saveEditsBusy.value || !edit.editCount.value) return;
  saveEditsBusy.value = true;
  try {
    const payload = {
      packageId: props.packageId,
      tgi: props.tgi,
      assetName: session.value?.assetName ?? null,
      transforms: [...edit.overrides.entries()],
      fields: [...edit.fieldOverrides.entries()],
    };
    const json = JSON.stringify(payload, null, 2);
    let binary = "";
    for (const byte of new TextEncoder().encode(json))
      binary += String.fromCharCode(byte);
    const path = await tauriApi.packages.saveFile(
      `${session.value?.assetName ?? "lot"}-edits.json`,
      "json",
    );
    if (!path) return;
    await command("write_export_file", {
      request: { path, dataBase64: btoa(binary) },
    });
  } finally {
    saveEditsBusy.value = false;
  }
}

// scoped 热键：sheet 打开且会话就绪时生效（输入框内不触发，Esc 除外）
useEditorHotkeys(
  computed(() => open.value && Boolean(session.value)),
  () => [
    { combo: "g", handler: () => setTool("translate") },
    { combo: "r", handler: () => setTool("rotate") },
    { combo: "s", handler: () => setTool("scale") },
    {
      combo: "escape",
      handler: () => {
        if (tool.value !== "select") setTool("select");
        else selectedId.value = null;
      },
    },
    { combo: "ctrl+z", handler: () => edit.undo() },
    { combo: "ctrl+shift+z", handler: () => edit.redo() },
  ],
);

const renderMode = ref<"default" | "refined">("default");
/** 动态招牌开关（2026-10-05 十轮，顶部工具条复选框）：默认关 = 静态
 * 量化合成招牌；开 = dev LED 扫掠动画（viewport prop 直下）。 */
const neonAnim = ref(false);

// 退出复位（2026-10-04 问题2）：sheet 关闭时把视图状态收回默认——组件
// 始终挂载（v-model:open），否则 renderMode/图层显隐跨会话残留，下次打开
// 直接全量重建精细渲染（首帧卡顿）或带着上次隐藏的图层。时段/LOD/编辑
// 数据保留（用户明确调节/资产），仅复位"视图"维度。
watch(open, (isOpen, wasOpen) => {
  if (isOpen || !wasOpen) return;
  renderMode.value = "default";
  neonAnim.value = false;
  tool.value = "select";
  resetViewState();
  // 卸载 prop 解析自动注册的 EcoGame 包（会话范围=注册范围）
  releasePropPackages();
});
/** 通道实验（已停用，见模板注释）：保留状态供复验时恢复。 */
const specExperiment = ref(false);
/** 0=自动逐像素 / 1=强制 G / 2=强制 B（精细渲染现固定 2）。 */
const specMode = ref(2);
/** 5d 日/夜时段 0–24（默认 12 正午）。 */
const timeOfDay = ref(12);
// 浮雕开关已撤销（见模板注释），reliefEnabled 状态一并移除。
const viewportRef = ref<{
  captureRender: (options?: { includeDecals?: boolean }) => string | null;
  groundPointAt: (
    clientX: number,
    clientY: number,
  ) => [number, number, number] | null;
} | null>(null);

/** 视口渲染图导出：剔除 gizmo 组后的视口截图（两种渲染模式均可用）。 */
const renderShotBusy = ref(false);
async function exportRenderImage() {
  if (renderShotBusy.value) return;
  renderShotBusy.value = true;
  try {
    // 精细模式下 decal 是投影到墙面的真实内容，导出保留（供像素核验与出图）
    const dataUrl = viewportRef.value?.captureRender({
      includeDecals: renderMode.value === "refined",
    });
    if (!dataUrl) return;
    const path = await tauriApi.packages.saveFile(
      `${session.value?.assetName ?? "lot"}-render.png`,
      "png",
    );
    if (!path) return;
    await command("write_export_file", {
      request: {
        path,
        dataBase64: dataUrl.slice("data:image/png;base64,".length),
      },
    });
  } finally {
    renderShotBusy.value = false;
  }
}

/** 模型导出（GLB）：默认模式仅白模；精细模式可选带贴图。 */
const meshExportBusy = ref(false);
async function exportModel(mode: "white" | "textured") {
  if (meshExportBusy.value) return;
  const lod = modelLods.value[activeLod.value];
  if (!lod) return;
  meshExportBusy.value = true;
  try {
    await exportLotModel({
      packageId: lod.packageId,
      modelTgi: lod.tgi,
      mode,
      defaultName:
        session.value?.assetName ??
        `0x${lod.tgi.instance.toString(16).padStart(8, "0")}`,
    });
  } finally {
    meshExportBusy.value = false;
  }
}
/** 5d 供电（断电 = 内景自发光全灭）。 */
const powered = ref(true);

watch(open, (value) => {
  if (value) {
    diagnosticsDismissed = false;
    void load();
  } else {
    // 关闭即回锁定态：工具栏/物料面板/拖放/删除随 editEnabled 复位
    // （用户资产编辑——放置记录/变换/字段——按既有口径不在复位范围）
    editEnabled.value = false;
    materialsOpen.value = false;
  }
});
/** 吸顶诊断横幅（FAlert）的会话内关闭状态：重开会话恢复显示。 */
let diagnosticsDismissed = false;
const title = computed(() => {
  const assetName = session.value?.assetName;
  if (assetName) return assetName;
  return `0x${props.tgi.instance.toString(16).padStart(8, "0").toUpperCase()}`;
});
const diagnostics = computed(() => session.value?.diagnostics ?? []);

/**
 * 左侧工作台 rail（低代码引擎布局对齐）：组件树收进可开合的 sheet——
 * rail 大纲按钮开关；图钉切换 停靠（占布局列）/悬浮（盖在视口上）。
 * 物料/源码与提交 Issue 为后续版本入口，当前禁用置灰。
 */
const treeSheetOpen = ref(true);
const treeSheetPinned = ref(true);
</script>

<template>
  <FSheet
    v-model:open="open"
    :label="$t('package.propertyEditor')"
    width="100vw"
  >
    <div v-if="open" class="editor-root">
      <!-- 吸顶诊断横幅：会话诊断一次展示，可关闭（重开会话恢复） -->
      <FAlert
        v-if="diagnostics.length && !diagnosticsDismissed"
        class="editor-alert"
        type="info"
        closable
        :close-label="$t('shell.close')"
        @close="diagnosticsDismissed = true"
      >
        {{ diagnostics.join(" · ") }}
      </FAlert>
      <header class="editor-header">
        <div class="editor-title">
          <strong>{{ $t("package.propertyEditor") }}</strong>
          <small>{{ title }}</small>
        </div>
        <div
          class="render-mode"
          role="group"
          :aria-label="$t('package.renderMode')"
          :title="$t('package.renderModeHint')"
        >

          <button
            type="button"
            :class="{ active: renderMode === 'default' }"
            @click="renderMode = 'default'"
          >
            {{ $t("package.renderModeDefault") }}
          </button>
          <button
            type="button"
            :class="{ active: renderMode === 'refined' }"
            @click="renderMode = 'refined'"
          >
            {{ $t("package.renderModeRefined") }}
          </button>
        </div>
        <!-- 通道实验（2026-09-10 停用）：经多 package 比对，B-窗通道综合
             效果最佳，精细渲染已在 Viewport 固定 specMode=2。复验时恢复
             此块与 specExperiment/specMode 的 prop 传递即可。
        <label
          v-if="renderMode === 'refined'"
          class="spec-experiment"
          :title="$t('package.specExperimentHint')"
        >
          <FCheckbox v-model="specExperiment" />
          <span>{{ $t("package.specExperiment") }}</span>
        </label>
        <div
          v-if="renderMode === 'refined' && specExperiment"
          class="render-mode spec-channel"
          role="group"
          :aria-label="$t('package.specExperiment')"
        >
          <button
            type="button"
            :class="{ active: specMode === 0 }"
            @click="specMode = 0"
          >
            {{ $t("package.specModeAuto") }}
          </button>
          <button
            type="button"
            :class="{ active: specMode === 1 }"
            @click="specMode = 1"
          >
            {{ $t("package.specModeG") }}
          </button>
          <button
            type="button"
            :class="{ active: specMode === 2 }"
            @click="specMode = 2"
          >
            {{ $t("package.specModeB") }}
          </button>
        </div>
        -->
        <div
          v-if="renderMode === 'refined'"
          class="daynight"
          :title="$t('package.timeOfDayHint')"
        >
          <span class="daynight-label">{{ $t("package.timeOfDay") }}</span>
          <input
            v-model.number="timeOfDay"
            type="range"
            min="0"
            max="24"
            step="0.5"
            :aria-label="$t('package.timeOfDay')"
          />
          <span class="daynight-value">
            {{ String(Math.floor(timeOfDay)).padStart(2, "0") }}:{{
              timeOfDay % 1 >= 0.5 ? "30" : "00"
            }}
          </span>
        </div>
        <!-- 浮雕开关（2026-09-10 撤销）：实证 slot5 alpha = 逐窗灯亮掩码而非
             几何高度（用户实测玻璃出现规律块纹），且该发行版 shader 的
             reliefMap() 已被编译为恒等——游戏本体无浮雕，数据无高度源。
             LOTM v8 的 reliefPng 字段保留（若未来找到真实高度图可恢复）。
        <label
          v-if="renderMode === 'refined'"
          class="spec-experiment"
          :title="$t('package.reliefHint')"
        >
          <FCheckbox v-model="reliefEnabled" />
          <span>{{ $t("package.relief") }}</span>
        </label>
        -->
        <label
          v-if="renderMode === 'refined'"
          class="spec-experiment"
          :title="$t('package.poweredHint')"
        >
          <FCheckbox v-model="powered" />
          <span>{{ $t("package.powered") }}</span>
        </label>
        <!-- 动态招牌（2026-10-05 十轮）：关 = 静态量化合成；开 = dev LED
             扫掠动画（decalNeonTubeSDF 双相位口径，§12.5） -->
        <label
          v-if="renderMode === 'refined'"
          class="spec-experiment"
          :title="$t('package.neonAnimHint')"
        >
          <FCheckbox v-model="neonAnim" />
          <span>{{ $t("package.neonAnim") }}</span>
        </label>
        <button
          class="editor-close"
          type="button"
          :disabled="saveEditsBusy || !edit.editCount.value"
          :aria-label="$t('package.saveEdits')"
          :title="$t('package.saveEditsHint')"
          @click="saveLocalEdits"
        >
          <FIcon
            :name="saveEditsBusy ? 'Loader2' : 'Save'"
            :size="15"
            aria-label=""
          />
        </button>
        <button
          class="editor-close"
          type="button"
          :disabled="renderShotBusy"
          :aria-label="$t('package.exportRender')"
          :title="$t('package.exportRender')"
          @click="exportRenderImage"
        >
          <FIcon
            :name="renderShotBusy ? 'Loader2' : 'Camera'"
            :size="15"
            aria-label=""
          />
        </button>
        <FDropdown :width="200">
          <template #trigger>
            <button
              class="editor-close"
              type="button"
              :disabled="meshExportBusy"
              :aria-label="$t('package.exportMesh')"
            >
              <FIcon
                :name="meshExportBusy ? 'Loader2' : 'Download'"
                :size="15"
                aria-label=""
              />
            </button>
          </template>
          <button type="button" @click="exportModel('white')">
            <FIcon name="Box" :size="14" aria-label="" />
            {{ $t("package.exportMeshWhite") }}
          </button>
          <button
            v-if="renderMode === 'refined'"
            type="button"
            @click="exportModel('textured')"
          >
            <FIcon name="Image" :size="14" aria-label="" />
            {{ $t("package.exportMeshTextured") }}
          </button>
        </FDropdown>
        <!-- 编辑模式锁定开关（原"只读视图"pill 的交互化）：解锁 = 工具栏/
             物料/拖放放置/删除可用 -->
        <FTooltip
          :text="
            editEnabled ? $t('package.editLock') : $t('package.editUnlock')
          "
          side="bottom"
        >
          <template #trigger>
            <button
              type="button"
              class="editor-close"
              :class="{ active: editEnabled }"
              :aria-pressed="editEnabled"
              :aria-label="
                editEnabled ? $t('package.editLock') : $t('package.editUnlock')
              "
              @click="toggleEditEnabled"
            >
              <FIcon :name="editEnabled ? 'LockOpen' : 'Lock'" :size="15" aria-label="" />
            </button>
          </template>
        </FTooltip>
        <span v-if="edit.editCount.value" class="editor-readonly editor-edits">
          {{ $t("package.localEdits", { n: edit.editCount.value }) }}
        </span>
        <button
          class="editor-close"
          type="button"
          :aria-label="$t('shell.close')"
          @click="open = false"
        >
          <FIcon name="X" :size="15" aria-label="" />
        </button>
      </header>
      <div v-if="loading" class="editor-loading">
        <FSpinner size="sm" :label="$t('common.loading')" />
      </div>
      <p v-else-if="loadError" class="editor-error" role="alert">
        {{ $t(loadError) }}
      </p>
      <div v-else-if="session" class="editor-body">
        <!-- 左侧工作台 rail：大纲（组件树 sheet 开关）/物料/源码（禁用）
             + 底部提交 Issue（禁用）——布局对齐低代码引擎。 -->
        <nav class="editor-rail" :aria-label="$t('package.inspector')">
          <div class="rail-group">
            <FTooltip :text="$t('package.railOutline')" side="right">
              <template #trigger>
                <button
                  type="button"
                  class="rail-item"
                  :class="{ active: treeSheetOpen }"
                  :aria-pressed="treeSheetOpen"
                  :aria-label="$t('package.railOutline')"
                  @click="treeSheetOpen = !treeSheetOpen"
                >
                  <FIcon name="ListTree" :size="17" aria-label="" />
                </button>
              </template>
            </FTooltip>
            <FTooltip
              :text="
                editEnabled
                  ? $t('package.materialsPanel')
                  : $t('package.editUnlockFirst')
              "
              side="right"
            >
              <template #trigger>
                <span class="rail-item-wrap">
                  <button
                    type="button"
                    class="rail-item"
                    :class="{ active: materialsOpen }"
                    :disabled="!editEnabled"
                    :aria-label="$t('package.materialsPanel')"
                    @click="openMaterialsPanel"
                  >
                    <FIcon name="Boxes" :size="17" aria-label="" />
                  </button>
                </span>
              </template>
            </FTooltip>
            <FTooltip :text="$t('package.railSource')" side="right">
              <template #trigger>
                <button
                  type="button"
                  class="rail-item"
                  :aria-label="$t('package.railSource')"
                  @click="schemaOpen = true"
                >
                  <FIcon name="CodeXml" :size="17" aria-label="" />
                </button>
              </template>
            </FTooltip>
          </div>
          <div class="rail-group">
            <FTooltip :text="$t('package.railSubmitIssue')" side="right">
              <template #trigger>
                <span class="rail-item-wrap">
                  <button
                    type="button"
                    class="rail-item"
                    disabled
                    :aria-label="$t('package.railSubmitIssue')"
                  >
                    <FIcon name="Send" :size="17" aria-label="" />
                  </button>
                </span>
              </template>
            </FTooltip>
          </div>
        </nav>
        <!-- 组件树 sheet：图钉=停靠（占布局列），未图钉=悬浮盖在视口上。
             dock 恒驻（空占位保持 grid 四列对位）——v-if 移除会让后续子元素
             左移错列，检查器落进 auto 列被内容撑爆（真机 bug 2026-10-06）。 -->
        <div
          class="tree-dock"
          :class="{ float: !treeSheetPinned || !treeSheetOpen }"
        >
          <section
            v-if="treeSheetOpen"
            class="tree-sheet"
            :class="{ overlay: !treeSheetPinned }"
            :aria-label="$t('package.componentTree')"
          >
            <header class="tree-sheet-head">
              <strong>{{ $t("package.componentTree") }}</strong>
              <div class="tree-sheet-actions">
                <button
                  type="button"
                  class="tree-sheet-btn"
                  :class="{ active: treeSheetPinned }"
                  :aria-pressed="treeSheetPinned"
                  :title="$t('package.pinSheet')"
                  @click="treeSheetPinned = !treeSheetPinned"
                >
                  <FIcon :name="treeSheetPinned ? 'Pin' : 'PinOff'" :size="13" aria-label="" />
                </button>
                <button
                  type="button"
                  class="tree-sheet-btn"
                  :title="$t('package.closeSheet')"
                  @click="treeSheetOpen = false"
                >
                  <FIcon name="X" :size="13" aria-label="" />
                </button>
              </div>
            </header>
            <PropertyEditorOutliner
              :grouping="grouping"
              :selected-id="selectedId"
              :hidden-units="hiddenUnits"
              :group-visibility="groupVisibility"
              @select="selectedId = $event"
              @toggle-unit="toggleUnit"
              @toggle-group="toggleGroup"
            />
          </section>
        </div>
        <PropertyEditorViewport
          ref="viewportRef"
          :edit-enabled="editEnabled"
          :added-model-payloads="addedModelPayloads"
          :pending-select-id="pendingSelectId"
          :resolve-added-model="resolveAddedModel"
          @place-unit="onPlaceUnit"
          @delete-unit="onDeleteUnit"
          :model-payload="modelPayload"
          :prop-models="propModels"
          :prop-tree-ids="propTreeIds"
          :tree-model-payloads="treeModelPayloads"
          :tree-atlas-png="treeAtlasPng"
          :model-lods="modelLods"
          :active-lod="activeLod"
          :render-mode="renderMode"
          :grouping="grouping"
          :lot-size="lotSize"
          :lot-tile-period="lotTilePeriod"
          :lot-placement="lotPlacement"
          :lot-colors="lotColors"
          :lot-colors-authored="lotColorsAuthored"
          :lot-border-colors="lotBorderColors"
          :lot-border-widths="lotBorderWidths"
          :lot-border-pattern-indices="lotBorderPatternIndices"
          :lot-base-tile="lotBaseTile"
          :lot-overlay-box-offset="lotOverlayBoxOffset"
          :lot-model-bbox-center="lotModelBBoxCenter"
          :lot-mask-png="lotMaskPng"
          :lot-mask-raw-rgba="lotMaskRawRgba"
          :lot-albedo-png="lotAlbedoPng"
          :lot-surface-png="lotSurfacePng"
          :lot-normal-atlas-png="lotNormalAtlasPng"
          :decal-textures="session?.decalTextures ?? []"
          :selected-id="selectedId"
          :hidden-units="hiddenUnits"
          :group-visibility="groupVisibility"
          :model-state="modelState"
          :spec-experiment="specExperiment"
          :spec-mode="specMode"
          :time-of-day="timeOfDay"
          :powered="powered"
          :neon-anim="neonAnim"
          :decal-light="decalLight"
          :tool="tool"
          @select="selectedId = $event"
          @toggle-layer="toggleGroup"
          @switch-lod="switchLod"
          @select-tool="setTool"
          @commit-transform="commitTransform"
          @live-transform="onLiveTransform"
        />
        <PropertyEditorInspector
          :unit="selectedUnit"
          :live-transform="liveTransform"
          :scale-editable="selectedScaleEditable"
          @update-transform="commitTransform"
          @update-fields="commitFields"
        />
      </div>
      <PropertyEditorStatusBar
        v-if="session"
        :grouping="grouping"
        :model-state="modelState"
        :selected-unit="selectedUnit"
      />
      <FSheet
        v-model:open="materialsOpen"
        :label="$t('package.materialsPanel')"
        width="min(420px, 92vw)"
      >
        <div class="materials-body">
          <div class="materials-tabs" role="tablist">
            <button
              v-for="tab in [
                { id: 'props', label: 'package.groupProps' },
                { id: 'spawners', label: 'package.groupSpawners' },
                { id: 'effects', label: 'package.groupEffects' },
                { id: 'paths', label: 'package.groupPaths' },
                { id: 'lights', label: 'package.groupLights' },
              ]"
              :key="tab.id"
              type="button"
              role="tab"
              class="materials-tab"
              :class="{ active: materialsTab === tab.id }"
              :aria-selected="materialsTab === tab.id"
              @click="materialsTab = tab.id"
            >
              {{ $t(tab.label) }}
            </button>
          </div>
          <input
            v-if="materialsTab === 'props'"
            v-model="materialsSearch"
            class="materials-search"
            type="search"
            :placeholder="$t('package.materialsSearch')"
          />
          <div class="materials-groups">
            <template v-if="materialsTab === 'props'">
              <div
                v-for="group in catalogGroups"
                :key="group.name"
                class="materials-group"
              >
                <button
                  type="button"
                  class="materials-group-head"
                  @click="toggleGroupCollapse(group.name)"
                >
                  <span>{{ group.name }}</span>
                  <FIcon
                    :name="
                      collapsedGroups.has(group.name) ? 'ChevronUp' : 'ChevronDown'
                    "
                    :size="13"
                    aria-label=""
                  />
                </button>
                <div
                  v-show="!collapsedGroups.has(group.name)"
                  class="materials-grid"
                >
                  <div
                    v-for="entry in group.entries"
                    :key="`${entry.packageId}:${entry.instance}`"
                    class="materials-entry"
                    @pointerdown.stop.prevent="
                      beginPlacementDrag($event, {
                        kind: 'prop',
                        packageId: entry.packageId,
                        tgi: {
                          typeId: 0x2f4e681b,
                          group: 0,
                          instance: entry.instance,
                        },
                        name: entry.name,
                      })
                    "
                  >
                    <FIcon name="Box" :size="20" aria-label="" />
                    <span>{{ entry.name }}</span>
                  </div>
                </div>
              </div>
              <p v-if="!catalogGroups.length" class="materials-empty">
                {{ $t('package.materialsEmpty') }}
              </p>
            </template>
            <template v-else-if="materialsTab === 'lights'">
              <div class="materials-grid">
                <div
                  v-for="preset in [
                    { lightType: 'Point', label: 'package.lightPoint' },
                    { lightType: 'Spot', label: 'package.lightSpot' },
                    { lightType: 'Line', label: 'package.lightLine' },
                  ]"
                  :key="preset.lightType"
                  class="materials-entry"
                  @pointerdown.stop.prevent="
                    beginPlacementDrag($event, {
                      kind: 'light',
                      lightType: preset.lightType,
                    })
                  "
                >
                  <FIcon name="Lightbulb" :size="20" aria-label="" />
                  <span>{{ $t(preset.label) }}</span>
                </div>
              </div>
            </template>
            <template v-else>
              <div class="materials-grid">
                <div
                  v-for="entry in [
                    {
                      tab: 'spawners',
                      kind: 'spawner',
                      label: 'package.groupSpawners',
                      icon: 'MapPin',
                    },
                    {
                      tab: 'effects',
                      kind: 'effect',
                      label: 'package.groupEffects',
                      icon: 'Zap',
                    },
                    {
                      tab: 'paths',
                      kind: 'pathPoint',
                      label: 'package.groupPaths',
                      icon: 'Spline',
                    },
                  ]"
                  v-show="materialsTab === entry.tab"
                  :key="entry.tab"
                  class="materials-entry"
                  @pointerdown.stop.prevent="
                    beginPlacementDrag($event, { kind: entry.kind })
                  "
                >
                  <FIcon :name="entry.icon" :size="20" aria-label="" />
                  <span>{{ $t(entry.label) }}</span>
                </div>
              </div>
            </template>
          </div>
        </div>
      </FSheet>
      <FSheet
        v-model:open="schemaOpen"
        :label="$t('package.schemaSheet')"
        width="72vw"
      >
        <div class="schema-sheet-body">
          <div class="schema-toolbar">
            <span class="schema-hint">{{ $t("package.schemaHint") }}</span>
            <button
              type="button"
              class="schema-export"
              :class="{ active: schemaDiff }"
              :aria-pressed="schemaDiff"
              :title="$t('package.schemaDiff')"
              @click="schemaDiff = !schemaDiff"
            >
              <FIcon name="GitCompare" :size="13" aria-label="" />
              {{ $t("package.schemaDiff") }}
            </button>
            <button
              type="button"
              class="schema-export"
              :title="$t('package.schemaRefresh')"
              @click="schemaRevision++"
            >
              <FIcon name="RefreshCw" :size="13" aria-label="" />
              {{ $t("package.schemaRefresh") }}
            </button>
            <button
              type="button"
              class="schema-export"
              :disabled="!schemaJson"
              @click="exportSchema"
            >
              <FIcon name="Download" :size="13" aria-label="" />
              {{ $t("package.schemaExport") }}
            </button>
          </div>
          <FCode
            :code="schemaJson"
            :base-code="schemaDiff ? schemaBaseJson : ''"
            line-numbers
            lang="json"
            copy-label="Copy"
            copied-label="Copied"
            class="schema-code"
          />
        </div>
      </FSheet>
    </div>
  </FSheet>
</template>

<style scoped>
.editor-root {
  /* 条件渲染的诊断条/加载/错误会打乱 grid 行序，flex 列与子元素数量无关 */
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.editor-header {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 12px;
  padding: 10px 16px;
}
.editor-title {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}
.editor-title strong {
  font-size: 14px;
}
.editor-title small {
  color: var(--subtle-foreground);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.editor-readonly {
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--subtle-foreground);
  font-size: 11px;
  padding: 3px 10px;
  white-space: nowrap;
}
.editor-edits {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  color: var(--accent);
}
/* 右侧聚拢由首个 .render-mode 的 auto margin 独立承担：
 * 中间插入的通道实验开关（label/div）不应断开推挤关系 */
.render-mode {
  display: inline-flex;
  margin-inline-start: auto;
}
.render-mode button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  min-height: 24px;
  padding: 2px 10px;
}
.render-mode button:first-child {
  border-end-end-radius: 0;
  border-start-end-radius: 0;
}
.render-mode button:last-child {
  border-end-start-radius: 0;
  border-start-start-radius: 0;
  margin-inline-start: -1px;
}
.render-mode button.active {
  color: var(--foreground);
  opacity: 0.85;
}
.render-mode.spec-channel {
  margin-inline-start: 0;
}
.spec-experiment {
  align-items: center;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font-size: 11px;
  gap: 6px;
  white-space: nowrap;
}
.matbase-input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 11px;
  min-height: 24px;
  padding: 2px 6px;
  width: 56px;
}
.daynight {
  align-items: center;
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 11px;
  gap: 6px;
  white-space: nowrap;
}
.daynight input[type="range"] {
  width: 110px;
}
.daynight-value {
  font-variant-numeric: tabular-nums;
  min-width: 34px;
  text-align: right;
}
.editor-close {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 28px;
  min-width: 28px;
}
.editor-close:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
/* 吸顶诊断横幅：盖在滚动内容之上，随 editor-root 顶缘吸附 */
.editor-alert {
  flex: none;
  position: sticky;
  top: 0;
  z-index: 20;
}
.editor-loading {
  display: grid;
  flex: 1;
  justify-items: center;
  align-content: center;
  min-height: 0;
}
.editor-error {
  color: var(--danger);
  flex: 1;
  font-size: 12px;
  padding: 24px 16px;
  text-align: center;
}
.editor-body {
  position: relative;
  display: grid;
  flex: 1;
  /* rail | 组件树 sheet（停靠时占列，悬浮/关闭塌缩为 0）| 视口 | 检查器
     （360px 中文恰好，英文 Properties/Transform/Metadata/Render Telemetry
     四 tab 溢出——加宽至 384px，tab 栏同时允许换行兜底，2026-10-06） */
  grid-template-columns: 48px auto minmax(0, 1fr) 384px;
  min-height: 0;
}
@media (max-width: 960px) {
  .editor-body {
    grid-template-columns: 44px auto minmax(0, 1fr) 320px;
  }
}
/* 左侧工作台 rail：图标+微标签竖排，组间留白，底部组贴齐下缘 */
.editor-rail {
  border-inline-end: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 10px;
  justify-content: space-between;
  min-height: 0;
  padding: 8px 4px;
}
.rail-group {
  align-items: center;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.rail-item {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  justify-content: center;
  min-height: 34px;
  width: 40px;
}
.rail-item-wrap {
  display: inline-flex;
}
.rail-item:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--foreground);
}
.rail-item.active {
  background: var(--accent);
  color: var(--foreground);
}
.rail-item:disabled {
  color: var(--subtle-foreground);
  cursor: not-allowed;
  opacity: 0.55;
}
/* 组件树 sheet 停靠列：悬浮时塌缩为 0（sheet 转绝对定位于 body） */
.tree-dock {
  display: flex;
  min-height: 0;
  min-width: 0;
}
.tree-dock.float {
  width: 0;
}
.tree-sheet {
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
  width: 232px;
}
.tree-sheet.overlay {
  background: var(--surface);
  box-shadow: 0 8px 28px rgb(0 0 0 / 35%);
  inset-inline-start: 48px;
  position: absolute;
  top: 0;
  bottom: 0;
  z-index: 15;
}
.tree-sheet-head {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  gap: 6px;
  padding: 7px 8px 7px 12px;
}
.tree-sheet-head strong {
  font-size: 12px;
  margin-inline-end: auto;
}
.tree-sheet-actions {
  align-items: center;
  display: flex;
  gap: 4px;
}
.tree-sheet-btn {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 22px;
  min-width: 22px;
  padding: 0;
}
.tree-sheet-btn:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.tree-sheet-btn.active {
  color: var(--accent);
}
/* Outliner 自带的分隔线在 sheet 内是双边框，剥掉并占满剩余高度 */
.schema-sheet-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding: 12px 16px;
}
.schema-toolbar {
  align-items: center;
  display: flex;
  gap: 10px;
}
.schema-hint {
  color: var(--subtle-foreground);
  flex: 1;
  font-size: 11px;
}
.schema-export {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 5px;
  min-height: 28px;
  padding: 0 10px;
}
.schema-export:hover {
  background: var(--surface-hover);
}
/* 变更对比开关激活态：品牌描边 + 浅底（与 rail 按钮同语义） */
.schema-export.active {
  background: var(--accent);
  border-color: var(--brand);
  color: var(--brand);
}
.schema-export:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.schema-code {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.materials-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding: 12px 14px;
}
.materials-tabs {
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 2px;
}
.materials-tab {
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  padding: 7px 10px 6px;
}
.materials-tab:hover {
  color: var(--foreground);
}
.materials-tab.active {
  border-bottom-color: var(--brand);
  color: var(--brand);
  font-weight: 600;
}
.materials-search {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  min-height: 30px;
  padding: 0 10px;
}
.materials-groups {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.materials-group {
  margin-bottom: 8px;
}
.materials-group-head {
  align-items: center;
  background: var(--surface-elevated);
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  font-weight: 600;
  justify-content: space-between;
  min-height: 30px;
  padding: 0 8px;
  width: 100%;
}
.materials-grid {
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(3, 1fr);
  margin-top: 6px;
}
.materials-entry {
  align-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: grab;
  display: flex;
  flex-direction: column;
  gap: 5px;
  color: var(--muted-foreground);
  font-size: 10px;
  min-height: 64px;
  justify-content: center;
  overflow: hidden;
  padding: 8px 4px;
  text-align: center;
}
.materials-entry:hover {
  border-color: var(--brand);
  color: var(--brand);
}
.materials-entry:active {
  cursor: grabbing;
}
.materials-entry span {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.materials-empty {
  color: var(--subtle-foreground);
  font-size: 11px;
  text-align: center;
}
.editor-close.active {
  border-color: var(--brand);
  color: var(--brand);
}
.tree-sheet :deep(.outliner) {
  border-inline-end: 0;
  flex: 1;
  min-height: 0;
}
</style>
