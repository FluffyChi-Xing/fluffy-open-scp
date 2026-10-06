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
import type { Tgi } from "@/api/tauri";
import PropertyEditorOutliner from "./PropertyEditorOutliner.vue";
import PropertyEditorViewport, {
  type EditorTool,
} from "./PropertyEditorViewport.vue";
import PropertyEditorInspector from "./PropertyEditorInspector.vue";
import PropertyEditorStatusBar from "./PropertyEditorStatusBar.vue";
import { usePropertyEditorSession } from "./usePropertyEditorSession";
import { useEditorHotkeys } from "./useEditorHotkeys";
import { exportLotModel } from "@/composables/useModelExport";
import { command } from "@/api/tauri";
import { tauriApi } from "@/api";

const props = defineProps<{ packageId: number; tgi: Tgi }>();
const open = defineModel<boolean>("open", { default: false });
const {
  session,
  loading,
  loadError,
  modelPayload,
  propModels,
  propTreeIds,
  treeAtlasPng,
  treeModelPayloads,
  releasePropPackages,
  modelLods,
  activeLod,
  switchLod,
  modelState,
  selectedId,
  grouping,
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
 * 显式保存：本地编辑导出为 JSON 补丁文件。当前编辑仅存在于内存，
 * 不触碰任何游戏 package；真正的 DBPF overlay 写回在资产-1（RW4
 * 写回器）落地后接入，届时同样以此按钮为唯一入口。
 */
// ---- Schema（低代码资产管线 v1，docs/design/lowcode-asset-pipeline.md）----
// 打开时把「当前 session + 编辑层覆盖」提交后端引擎，产出规范化
// openscp.lot-asset/1 JSON；FCode 只读预览，导出写 .lot.json。
const schemaOpen = ref(false);
const schemaBusy = ref(false);
const schemaJson = ref("");
async function openSchemaSheet() {
  if (schemaBusy.value || !session.value) return;
  schemaBusy.value = true;
  try {
    const overrides = [...edit.overrides.entries()].map(([id, matrix]) => ({
      id,
      matrix,
    }));
    const fieldPatches = [...edit.fieldOverrides.entries()].map(
      ([id, fields]) => ({ id, fields }),
    );
    const merged = new Map<string, { id: string; matrix?: number[]; fields?: Record<string, unknown> }>();
    for (const entry of [...overrides, ...fieldPatches]) {
      merged.set(entry.id, { ...merged.get(entry.id), ...entry });
    }
    const current = session.value;
    const request = {
      session: {
        assetName: current.assetName ?? null,
        tgi: props.tgi,
        modelLods: current.modelLods,
        lotSize: current.lotSize,
        lotTilePeriod: current.lotTilePeriod,
        lotPlacement: current.lotPlacement,
        lotColors: current.lotColors,
        lotColorsAuthored: current.lotColorsAuthored,
        lotBorderColors: current.lotBorderColors,
        lotBorderWidths: current.lotBorderWidths,
        lotBorderPatternIndices: current.lotBorderPatternIndices,
        lotBaseTile: current.lotBaseTile,
        lotOverlayBoxOffset: current.lotOverlayBoxOffset,
        lotModelBBoxCenter: current.lotModelBBoxCenter,
        units: current.units,
      },
      overrides: [...merged.values()],
      hiddenUnitIds: [...hiddenUnits.value],
      groups: { ...groupVisibility },
    };
    const result = await tauriApi.packages.buildPeSchema(request);
    schemaJson.value = result.schemaJson;
    schemaOpen.value = true;
  } finally {
    schemaBusy.value = false;
  }
}
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
        <span v-if="edit.editCount.value" class="editor-readonly editor-edits">
          {{ $t("package.localEdits", { n: edit.editCount.value }) }}
        </span>
        <span v-else class="editor-readonly">{{
          $t("package.propertyEditorReadonly")
        }}</span>
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
            <FTooltip :text="$t('package.railMaterials')" side="right">
              <template #trigger>
                <span class="rail-item-wrap">
                  <button
                    type="button"
                    class="rail-item"
                    disabled
                    :aria-label="$t('package.railMaterials')"
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
                  @click="openSchemaSheet"
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
              :disabled="!schemaJson"
              @click="exportSchema"
            >
              <FIcon name="Download" :size="13" aria-label="" />
              {{ $t("package.schemaExport") }}
            </button>
          </div>
          <FCode
            :code="schemaJson"
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
  /* rail | 组件树 sheet（停靠时占列，悬浮/关闭塌缩为 0）| 视口 | 检查器 */
  grid-template-columns: 48px auto minmax(0, 1fr) 360px;
  min-height: 0;
}
@media (max-width: 960px) {
  .editor-body {
    grid-template-columns: 44px auto minmax(0, 1fr) 300px;
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
.schema-export:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.schema-code {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.tree-sheet :deep(.outliner) {
  border-inline-end: 0;
  flex: 1;
  min-height: 0;
}
</style>
