<script setup lang="ts">
import { computed, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FCheckbox from "@/components/ui/FCheckbox.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FSheet from "@/components/ui/FSheet.vue";
import type { Tgi } from "@/api/tauri";
import PropertyEditorOutliner from "./PropertyEditorOutliner.vue";
import PropertyEditorViewport, { type EditorTool } from "./PropertyEditorViewport.vue";
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
  modelLods,
  activeLod,
  switchLod,
  modelState,
  selectedId,
  grouping,
  lotSize,
  lotPlacement,
  lotColors,
  lotColorsAuthored,
  lotMaskPng,
  lotMaskRawRgba,
  lotAlbedoPng,
  lotSurfacePng,
  selectedUnit,
  edit,
  hiddenUnits,
  groupVisibility,
  load,
  toggleGroup,
  toggleUnit,
} = usePropertyEditorSession(props.packageId, props.tgi);

/** 编辑工具（PE-重构-3）：select = 仅拾取；translate/rotate/scale 挂手柄。 */
const tool = ref<EditorTool>("select");
function setTool(next: EditorTool) {
  tool.value = next;
}

/** 手柄拖拽中的实时变换（坐标面板即时显示；id 为 null = 拖拽结束）。 */
const liveTransform = ref<{ id: string; position: [number, number, number] } | null>(
  null,
);
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
    for (const byte of new TextEncoder().encode(json)) binary += String.fromCharCode(byte);
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
/** 通道实验（已停用，见模板注释）：保留状态供复验时恢复。 */
const specExperiment = ref(false);
/** 0=自动逐像素 / 1=强制 G / 2=强制 B（精细渲染现固定 2）。 */
const specMode = ref(2);
/** 5d 日/夜时段 0–24（默认 12 正午）。 */
const timeOfDay = ref(12);
// 浮雕开关已撤销（见模板注释），reliefEnabled 状态一并移除。
const viewportRef = ref<{ captureRender: () => string | null } | null>(null);

/** 视口渲染图导出：剔除 gizmo 组后的视口截图（两种渲染模式均可用）。 */
const renderShotBusy = ref(false);
async function exportRenderImage() {
  if (renderShotBusy.value) return;
  renderShotBusy.value = true;
  try {
    const dataUrl = viewportRef.value?.captureRender();
    if (!dataUrl) return;
    const path = await tauriApi.packages.saveFile(
      `${session.value?.assetName ?? "lot"}-render.png`,
      "png",
    );
    if (!path) return;
    await command("write_export_file", {
      request: { path, dataBase64: dataUrl.slice("data:image/png;base64,".length) },
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
  if (value) void load();
});
const title = computed(() => {
  const assetName = session.value?.assetName;
  if (assetName) return assetName;
  return `0x${props.tgi.instance.toString(16).padStart(8, "0").toUpperCase()}`;
});
const diagnostics = computed(() => session.value?.diagnostics ?? []);
</script>

<template>
  <FSheet v-model:open="open" :label="$t('package.propertyEditor')" width="100vw">
    <div v-if="open" class="editor-root">
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
          <FIcon :name="renderShotBusy ? 'Loader2' : 'Camera'" :size="15" aria-label="" />
        </button>
        <FDropdown :width="200">
          <template #trigger>
            <button class="editor-close" type="button" :disabled="meshExportBusy"
              :aria-label="$t('package.exportMesh')">
              <FIcon :name="meshExportBusy ? 'Loader2' : 'Download'" :size="15" aria-label="" />
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
        <span v-else class="editor-readonly">{{ $t("package.propertyEditorReadonly") }}</span>
        <button
          class="editor-close"
          type="button"
          :aria-label="$t('shell.close')"
          @click="open = false"
        >
          <FIcon name="X" :size="15" aria-label="" />
        </button>
      </header>
      <p v-if="diagnostics.length" class="editor-diagnostics">
        <FIcon name="TriangleAlert" :size="13" aria-label="" />
        <span>{{ diagnostics.join(" · ") }}</span>
      </p>
      <div v-if="loading" class="editor-loading">
        <FSpinner size="sm" :label="$t('common.loading')" />
      </div>
      <p v-else-if="loadError" class="editor-error" role="alert">
        {{ $t(loadError) }}
      </p>
      <div v-else-if="session" class="editor-body">
        <PropertyEditorOutliner
          :grouping="grouping"
          :selected-id="selectedId"
          :hidden-units="hiddenUnits"
          :group-visibility="groupVisibility"
          @select="selectedId = $event"
          @toggle-unit="toggleUnit"
          @toggle-group="toggleGroup"
        />
        <PropertyEditorViewport
          ref="viewportRef"
          :model-payload="modelPayload"
          :model-lods="modelLods"
          :active-lod="activeLod"
          :render-mode="renderMode"
          :grouping="grouping"
          :lot-size="lotSize"
          :lot-placement="lotPlacement"
          :lot-colors="lotColors"
          :lot-colors-authored="lotColorsAuthored"
          :lot-mask-png="lotMaskPng"
          :lot-mask-raw-rgba="lotMaskRawRgba"
          :lot-albedo-png="lotAlbedoPng"
          :lot-surface-png="lotSurfacePng"
          :selected-id="selectedId"
          :hidden-units="hiddenUnits"
          :group-visibility="groupVisibility"
          :model-state="modelState"
          :spec-experiment="specExperiment"
          :spec-mode="specMode"
          :time-of-day="timeOfDay"
          :powered="powered"
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
.editor-diagnostics {
  align-items: center;
  background: var(--surface-elevated);
  border-bottom: 1px solid var(--border);
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  gap: 7px;
  margin: 0;
  overflow: hidden;
  padding: 6px 16px;
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
  display: grid;
  flex: 1;
  grid-template-columns: 220px minmax(0, 1fr) 300px;
  min-height: 0;
}
@media (max-width: 960px) {
  .editor-body {
    grid-template-columns: 180px minmax(0, 1fr) 240px;
  }
}
</style>
