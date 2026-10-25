<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEventListener } from "@vueuse/core";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FSheet from "@/components/ui/FSheet.vue";
import FTooltip from "@/components/ui/FTooltip.vue";
import MonacoEditor from "@/components/ui/MonacoEditor.vue";
import { Switch } from "@/components/ui/switch";
import {
  Resizable,
  ResizableHandle,
  ResizablePanel,
} from "@/components/ui/resizable";
import { tauriApi } from "@/api";
import { command } from "@/api/tauri";
import type { Tgi } from "@/api/tauri";
import { useToast } from "@/composables/useToast";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import SchemaPropertyViewport from "@/pages/packages/components/property-editor/SchemaPropertyViewport.vue";
import PropertyEditorOutliner from "@/pages/packages/components/property-editor/PropertyEditorOutliner.vue";
import PropertyEditorInspector from "@/pages/packages/components/property-editor/PropertyEditorInspector.vue";
import PropertyEditorStatusBar from "@/pages/packages/components/property-editor/PropertyEditorStatusBar.vue";
import { useEffectPreview } from "@/pages/packages/components/property-editor/effects/useEffectPreview";
import { useEditorHotkeys } from "@/pages/packages/components/property-editor/useEditorHotkeys";
import {
  assetCategory,
  directModelKey,
} from "@/pages/packages/components/property-editor/assetCategories";
import { exportLotModel } from "@/composables/useModelExport";
import AssetSourceTree from "./AssetSourceTree.vue";
import AssetComponentLibrary from "./AssetComponentLibrary.vue";
import AssetLotPanel from "./AssetLotPanel.vue";
import AssetBlueprintCanvas from "./AssetBlueprintCanvas.vue";
import {
  useWorkbenchAsset,
  type LotPresetRef,
  type MaterialSlotBinding,
  type PlacePayload,
} from "./useWorkbenchAsset";
import AssetLotLibrary from "./AssetLotLibrary.vue";
import AssetMaterialSlotsPanel from "./AssetMaterialSlotsPanel.vue";

/**
 * Assets Mod 工作台（Blender 式三栏，2026-10-25 彻底重构）：
 *  左 = 项目文件树（导入/点击即开）；中上 = 完整 Property Editor 视口
 *  （坐标系/图层/工具栏/渲染状态全量保留）；中下 = 蓝图画布（资产 →
 *  组件 → 菜单绑定 → 打包 真实状态管线）；右 = 组件树/组件库/地块/
 *  Schema(Monaco) + 检查器（原 property editor 右侧四页签）。
 * 资产来源：GLB/LOTM 文件（Blender 六槽烘焙管线）或 TGI 直接解析游戏包。
 */
const props = defineProps<{ project: string }>();

const { t } = useI18n();
const toast = useToast();
const asset = useWorkbenchAsset(props.project);
const ui = asset.ui;

// ---- 顶部工具条状态（与 PropertyEditor 渲染控制同口径）----
const editEnabled = ref(true);
const tool = ref<"select" | "translate" | "rotate" | "scale">("select");
// 渲染模式自由切换：进入编辑模式时默认落在"默认渲染"（白模+标记更利于
// 摆放组件），用户随时可切"精细"看贴图效果——两种模式在编辑态都可用
// （用户指令 2026-10-25：默认≠禁用）。
const renderMode = ref<"default" | "refined">("default");
const effectiveRenderMode = computed(() => renderMode.value);
const timeOfDay = ref(12);
const powered = ref(true);
const reliefEnabled = ref(false);
const neonAnim = ref(false);
const effectRendering = ref(false);
const effectsActive = computed(
  () =>
    effectRendering.value &&
    effectiveRenderMode.value === "refined" &&
    ui.value !== null,
);
const effectUnits = computed(() => ui.value?.grouping.effects ?? []);
const effectPreview = useEffectPreview(
  () => asset.nativePackageId.value ?? 0,
  effectsActive,
  effectUnits,
);
const effectRenderTitle = computed(() =>
  t(
    effectPreview.failed.value
      ? "package.effectRenderingError"
      : effectPreview.loading.value
        ? "package.effectRenderingLoading"
        : effectPreview.limited.value
          ? "package.effectRenderingLimited"
          : "package.effectRenderingHint",
  ),
);

// ---- 选中/检查器联动 ----
const liveTransform = ref<{
  id: string;
  position: [number, number, number];
} | null>(null);
const selectedScaleEditable = computed(() => {
  const unit = ui.value?.selectedUnit;
  if (!unit || unit.kind !== "prop") return false;
  const rid = unit.resourceId;
  if (typeof rid !== "number") return false;
  return (
    (ui.value?.propTreeIds.has(rid) ?? false) ||
    (ui.value?.propModels.has(rid) ?? false) ||
    asset.addedModelPayloads.value.has(directModelKey(unit))
  );
});
function commitTransform(id: string, matrix: number[]) {
  // "model" = 草稿导入模型的伪单元：变换存模型通道（随文档持久化）
  if (id === "model") {
    asset.setModelTransform(matrix);
    return;
  }
  ui.value?.edit.setUnitTransform(id, matrix);
}

/** 编辑锁：解锁进入编辑时渲染模式默认落在"默认渲染"（可自由切精细）。 */
function toggleEdit() {
  editEnabled.value = !editEnabled.value;
  if (editEnabled.value) renderMode.value = "default";
}
function commitFields(id: string, patch: Record<string, unknown>) {
  ui.value?.edit.setUnitFields(id, patch);
}

// ---- 视口 props（schema 文档 = 单一数据源；事件显式绑定）----
const viewportRef = ref<InstanceType<typeof SchemaPropertyViewport> | null>(
  null,
);
const viewportBindings = computed(() => {
  const current = ui.value;
  if (!current) return null;
  return {
    schema: asset.schemaDocument.value,
    modelPayload: current.modelPayload,
    modelLods: current.modelLods,
    activeLod: current.activeLod,
    renderMode: effectiveRenderMode.value,
    modelState: current.modelState,
    lotMaskPng: current.lotMaskPng,
    lotMaskRawRgba: current.lotMaskRawRgba,
    lotAlbedoPng: current.lotAlbedoPng,
    lotSurfacePng: current.lotSurfacePng,
    lotNormalAtlasPng: current.lotNormalAtlasPng,
    propModels: current.propModels,
    propTreeIds: current.propTreeIds,
    treeModelPayloads: current.treeModelPayloads,
    treeAtlasPng: current.treeAtlasPng,
    addedModelPayloads: asset.addedModelPayloads.value,
    decalTextures: current.decalTextures,
    decalLight: current.decalLight,
    selectedId: current.selectedId.value,
    tool: tool.value,
    editEnabled: editEnabled.value,
    modelSelectable: editEnabled.value && current.editMode === "draft",
    modelTransform: current.modelTransform,
    modelOrigin: current.modelOrigin,
    specMode: 2,
    timeOfDay: timeOfDay.value,
    powered: powered.value,
    reliefEnabled: reliefEnabled.value,
    neonAnim: neonAnim.value,
    renderEffects: effectsActive.value,
    effectDefinitions: effectPreview.definitions.value,
    modelSource:
      asset.sourceKind.value === "glb" || asset.sourceKind.value === "lotm"
        ? ("glb" as const)
        : ("native" as const),
  };
});
const pendingSelectId = ref<string | null>(null);
watch(
  () => ui.value?.selectedId.value,
  (value) => {
    if (!value) pendingSelectId.value = null;
  },
);

// ---- 组件放置：库条目 pointerdown 拾起 → 视口地面落点放置 ----
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
  pendingSelectId.value = asset.placeUnit(payload, position);
});

// ---- 热键（sheet 级作用域同 PE）----
useEditorHotkeys(
  computed(() => ui.value !== null),
  () => [
    { combo: "g", handler: () => (tool.value = "translate") },
    { combo: "r", handler: () => (tool.value = "rotate") },
    { combo: "s", handler: () => (tool.value = "scale") },
    {
      combo: "escape",
      handler: () => {
        if (tool.value !== "select") tool.value = "select";
        else asset.selectUnit(null);
      },
    },
    { combo: "ctrl+z", handler: () => ui.value?.edit.undo() },
    { combo: "ctrl+shift+z", handler: () => ui.value?.edit.redo() },
  ],
);

// ---- 右栏页签 ----
type RightTab = "tree" | "library" | "lot" | "lotlib" | "slots" | "schema";
const rightTab = ref<RightTab>("tree");
const RIGHT_TABS: { id: RightTab; icon: string; label: string }[] = [
  { id: "tree", icon: "ListTree", label: "package.componentTree" },
  { id: "library", icon: "Boxes", label: "package.materialsPanel" },
  { id: "lot", icon: "LandPlot", label: "studio.assetWorkbench.lotTab" },
  { id: "lotlib", icon: "Map", label: "studio.assetWorkbench.lotLibraryTab" },
  {
    id: "slots",
    icon: "Image",
    label: "studio.assetWorkbench.materialSlotsTab",
  },
  { id: "schema", icon: "CodeXml", label: "package.schemaSheet" },
];
async function onBindMaterialSlots(binding: MaterialSlotBinding) {
  try {
    await asset.composeSlotMaterials(binding);
    toast.success(t("studio.assetWorkbench.slotApplied"));
  } catch (cause) {
    toast.error(String(cause));
  }
}
async function onBlendImported(result: {
  asset: string;
  baked?: boolean;
  materialSlots?: Record<string, string>;
  materialSlotsByMaterial?: Record<string, Record<string, string>>;
}) {
  try {
    await asset.openDraftAsset(result.asset);
    if (result.baked) {
      toast.success(t("studio.assetWorkbench.blendImported"));
      return;
    }
    const slots = result.materialSlots ?? {};
    const binding: MaterialSlotBinding = {};
    for (const key of [
      "params",
      "materialParams",
      "slot1",
      "slot2",
      "slot3",
      "slot4",
      "slot5",
    ] as const) {
      const value = slots[key];
      if (typeof value === "string" && value) binding[key] = value;
    }
    const byMaterial = result.materialSlotsByMaterial ?? {};
    const materialBindings = Object.fromEntries(
      Object.entries(byMaterial).flatMap(([name, slots]) => {
        const entry: Record<string, string> = {};
        for (const key of ["slot1", "slot2", "slot3", "slot4", "slot5"] as const) {
          if (typeof slots[key] === "string" && slots[key]) entry[key] = slots[key];
        }
        return Object.keys(entry).length ? [[name, entry]] : [];
      }),
    );
    if (Object.keys(materialBindings).length) binding.byMaterial = materialBindings;
    if (Object.keys(binding).length) {
      await asset.composeSlotMaterials(binding);
      toast.success(
        t("studio.assetWorkbench.blendImported", {
          count: Object.keys(binding).length,
        }),
      );
    } else {
      toast.info(t("studio.assetWorkbench.blendImportedNoSlots"));
    }
  } catch (cause) {
    toast.error(String(cause));
  }
}
const applyingLot = ref(false);
async function onApplyLotPreset(preset: LotPresetRef) {
  if (applyingLot.value) return;
  applyingLot.value = true;
  try {
    const size = await asset.applyLotPreset(preset);
    toast.success(
      size
        ? t("studio.assetWorkbench.lotAppliedToast", {
            width: size[0],
            depth: size[1],
          })
        : t("studio.assetWorkbench.lotApplied"),
    );
  } catch (cause) {
    const message = (cause as { message?: string }).message ?? String(cause);
    toast.error(
      message === "lot_apply_native"
        ? t("studio.assetWorkbench.lotApplyNativeHint")
        : message,
    );
  } finally {
    applyingLot.value = false;
  }
}

// ---- 导入：模型文件 / TGI 解析 ----
const treeRef = ref<InstanceType<typeof AssetSourceTree> | null>(null);
const tgiSheetOpen = ref(false);
const tgiPackagePath = ref("");
const tgiText = ref("");
const tgiBusy = ref(false);
const packageCandidates = ref<Tgi[]>([]);
/** 树 .package 点击 → 读资源表 → 列出 Lot property 候选并打开选择页。 */
async function onOpenPackage(relativePath: string) {
  packageCandidates.value = [];
  tgiPackagePath.value = relativePath;
  tgiText.value = "";
  tgiSheetOpen.value = true;
  try {
    await asset.resolveProjectRoot();
    const value = await command<{
      resources?: { typeId: number; group: number; instance: number }[];
    }>("code_flow", {
      request: {
        project: props.project,
        action: "read-asset",
        payload: { asset: relativePath },
      },
    });
    packageCandidates.value = (value.resources ?? [])
      .filter((resource) => resource.typeId === 0x00b1b104)
      .map((resource) => ({
        typeId: resource.typeId,
        group: resource.group,
        instance: resource.instance,
      }));
    if (!packageCandidates.value.length) {
      toast.warning(t("studio.assetWorkbench.tgiNoLotProperty"));
    }
  } catch (cause) {
    toast.error(String(cause));
  }
}
function parseTgiText(value: string): Tgi | null {
  const parts = value
    .split(/[:/,]/)
    .map((part) => Number.parseInt(part.trim().replace(/^0x/i, ""), 16));
  if (parts.length === 3 && parts.every(Number.isFinite))
    return { typeId: parts[0], group: parts[1], instance: parts[2] };
  return null;
}
async function submitTgi() {
  const tgi = parseTgiText(tgiText.value);
  if (!tgi || !tgiPackagePath.value) return;
  tgiBusy.value = true;
  try {
    await asset.openNative(tgiPackagePath.value, tgi);
    tgiSheetOpen.value = false;
    toast.success(
      t("studio.assetWorkbench.tgiOpened", { tgi: tgiText.value.trim() }),
    );
  } catch (cause) {
    toast.error(String(cause));
  } finally {
    tgiBusy.value = false;
  }
}
function openCandidate(tgi: Tgi) {
  tgiText.value = [tgi.typeId, tgi.group, tgi.instance]
    .map((value) => `0x${value.toString(16).padStart(8, "0")}`)
    .join(":");
}
async function browsePackage() {
  const path = await openDialog({
    multiple: false,
    filters: [
      {
        name: t("studio.assetWorkbench.tgiPackageFilter"),
        extensions: ["package"],
      },
    ],
  });
  if (typeof path === "string") {
    tgiPackagePath.value = path;
    packageCandidates.value = [];
  }
}
function hex32(value: number): string {
  return `0x${(value >>> 0).toString(16).padStart(8, "0").toUpperCase()}`;
}

// ---- 保存 / 导出 ----
const saving = ref(false);
const removing = ref(false);
async function onSave() {
  if (saving.value || !ui.value) return;
  saving.value = true;
  try {
    if (await asset.saveDocument()) {
      await treeRef.value?.refresh();
      toast.success(
        t("studio.assetWorkbench.saved", { path: asset.documentPath }),
      );
    }
  } catch (cause) {
    toast.error(String(cause));
  } finally {
    saving.value = false;
  }
}
async function exportSchema() {
  if (!asset.schemaJson.value) return;
  const path = await tauriApi.packages.saveFile(
    `${ui.value?.assetName ?? "lot"}.lot.json`,
    "json",
  );
  if (!path) return;
  const binary = JSON.stringify(JSON.parse(asset.schemaJson.value));
  let payload = "";
  for (const byte of new TextEncoder().encode(binary))
    payload += String.fromCharCode(byte);
  await command("write_export_file", {
    request: { path, dataBase64: btoa(payload) },
  });
  toast.success(t("studio.assetWorkbench.exportedSchema"));
}
const shotBusy = ref(false);
async function exportRender() {
  if (shotBusy.value) return;
  shotBusy.value = true;
  try {
    const dataUrl = await viewportRef.value?.captureRender({
      includeDecals: effectiveRenderMode.value === "refined",
    });
    if (!dataUrl) return;
    const path = await tauriApi.packages.saveFile(
      `${ui.value?.assetName ?? "lot"}-render.png`,
      "png",
    );
    if (!path) return;
    await command("write_export_file", {
      request: {
        path,
        dataBase64: dataUrl.slice("data:image/png;base64,".length),
      },
    });
    toast.success(t("studio.assetWorkbench.exportedRender"));
  } finally {
    shotBusy.value = false;
  }
}
const meshExportBusy = ref(false);
async function exportMesh(mode: "white" | "textured") {
  if (meshExportBusy.value) return;
  const lod = ui.value?.modelLods[ui.value?.activeLod ?? 0];
  if (!lod) return;
  meshExportBusy.value = true;
  try {
    await exportLotModel({
      packageId: lod.packageId,
      modelTgi: lod.tgi,
      mode,
      defaultName: ui.value?.assetName ?? "lot",
    });
  } finally {
    meshExportBusy.value = false;
  }
}

// ---- 蓝图画布数据 ----
const unitCounts = computed(() => {
  const grouping = ui.value?.grouping;
  return {
    lights: grouping?.lights.length ?? 0,
    props: grouping?.props.length ?? 0,
    effects: grouping?.effects.length ?? 0,
    spawners: grouping?.spawners.length ?? 0,
    paths: grouping?.pathPoints.length ?? 0,
  };
});

/** 移除当前资产（清空视口回到空态，可重新上传/解析）。 */
async function onRemoveAsset() {
  if (!asset.sourceKind.value) return;
  if (
    asset.savedDirty.value &&
    !window.confirm(t("studio.assetWorkbench.removeConfirm"))
  ) {
    return;
  }
  removing.value = true;
  try {
    await asset.removeAsset();
    void treeRef.value?.refresh();
    toast.info(t("studio.assetWorkbench.removed"));
  } catch (cause) {
    toast.error(String(cause));
  } finally {
    removing.value = false;
  }
}

onMounted(() => {
  void asset.restoreFromProject();
});

function templateOf(resourceId: number) {
  const reference = ui.value?.referencedModels.find(
    (item) => item.resourceId === resourceId,
  );
  if (!reference) return undefined;
  return ui.value?.flatUnits.find(
    (unit): unit is import("@/api/tauri").PropUnit =>
      unit.kind === "prop" && unit.resourceId === reference.resourceId,
  );
}
</script>

<template>
  <section class="maw" :aria-label="t('studio.assetWorkbench.title')">
    <header class="maw-toolbar">
      <div class="maw-title">
        <span class="maw-badge" :data-kind="asset.sourceKind.value">
          <FIcon
            :name="
              asset.sourceKind.value === 'native'
                ? 'PackageSearch'
                : asset.sourceKind.value === 'none'
                  ? 'Box'
                  : 'Box'
            "
            :size="12"
            aria-label=""
          />
          {{
            asset.sourceKind.value === "native"
              ? t("studio.assetWorkbench.sourceNative")
              : asset.sourceKind.value === "none"
                ? t("studio.assetWorkbench.sourceNone")
                : t("studio.assetWorkbench.sourceGlb")
          }}
        </span>
        <strong>{{
          asset.sourceLabel.value || t("studio.assetWorkbench.title")
        }}</strong>
        <span v-if="asset.savedDirty.value" class="maw-dirty">
          {{ t("studio.assetWorkbench.unsaved") }}
        </span>
      </div>

      <div class="maw-actions">
        <button type="button" class="maw-btn" @click="tgiSheetOpen = true">
          <FIcon name="Hash" :size="13" aria-label="" />
          {{ t("studio.assetWorkbench.tgiParse") }}
        </button>
        <template v-if="ui">
          <div
            class="maw-seg"
            role="group"
            :aria-label="t('package.renderMode')"
            :title="t('package.renderModeHint')"
          >
            <button
              type="button"
              :class="{ active: effectiveRenderMode === 'default' }"
              @click="renderMode = 'default'"
            >
              {{ t("package.renderModeDefault") }}
            </button>
            <button
              type="button"
              :class="{ active: effectiveRenderMode === 'refined' }"
              @click="renderMode = 'refined'"
            >
              {{ t("package.renderModeRefined") }}
            </button>
          </div>
          <label
            v-if="effectiveRenderMode === 'refined'"
            class="maw-daynight"
            :title="t('package.timeOfDayHint')"
          >
            <span>{{ t("package.timeOfDay") }}</span>
            <input
              v-model.number="timeOfDay"
              type="range"
              min="0"
              max="24"
              step="0.5"
              :aria-label="t('package.timeOfDay')"
            />
            <span class="maw-daynight-value">
              {{ String(Math.floor(timeOfDay)).padStart(2, "0") }}:{{
                timeOfDay % 1 >= 0.5 ? "30" : "00"
              }}
            </span>
          </label>
          <label
            v-if="effectiveRenderMode === 'refined'"
            class="maw-switch"
            :title="t('package.poweredHint')"
          >
            <Switch v-model="powered" :aria-label="t('package.powered')" />
            <span>{{ t("package.powered") }}</span>
          </label>
          <label
            v-if="effectiveRenderMode === 'refined'"
            class="maw-switch"
            :title="t('package.reliefHint')"
          >
            <Switch v-model="reliefEnabled" :aria-label="t('package.relief')" />
            <span>{{ t("package.relief") }}</span>
          </label>
          <label
            v-if="effectiveRenderMode === 'refined'"
            class="maw-switch"
            :title="t('package.neonAnimHint')"
          >
            <Switch v-model="neonAnim" :aria-label="t('package.neonAnim')" />
            <span>{{ t("package.neonAnim") }}</span>
          </label>
          <label
            v-if="
              asset.sourceKind.value === 'native' &&
              effectiveRenderMode === 'refined'
            "
            class="maw-switch"
            :title="effectRenderTitle"
          >
            <Switch
              v-model="effectRendering"
              :aria-label="t('package.effectRendering')"
            />
            <span>{{ t("package.effectRendering") }}</span>
          </label>

          <span class="maw-gap" />
          <FTooltip :text="t('package.undo')" side="bottom">
            <template #trigger>
              <button
                type="button"
                class="maw-icon"
                :disabled="!ui.edit.canUndo.value"
                :aria-label="t('package.undo')"
                @click="ui.edit.undo()"
              >
                <FIcon name="Undo2" :size="14" aria-label="" />
              </button>
            </template>
          </FTooltip>
          <FTooltip :text="t('package.redo')" side="bottom">
            <template #trigger>
              <button
                type="button"
                class="maw-icon"
                :disabled="!ui.edit.canRedo.value"
                :aria-label="t('package.redo')"
                @click="ui.edit.redo()"
              >
                <FIcon name="Redo2" :size="14" aria-label="" />
              </button>
            </template>
          </FTooltip>
          <button
            type="button"
            class="maw-btn primary"
            :disabled="saving || !asset.savedDirty.value"
            :title="t('studio.assetWorkbench.saveHint')"
            @click="onSave"
          >
            <FIcon
              :name="saving ? 'Loader2' : 'Save'"
              :size="13"
              aria-label=""
            />
            {{ t("studio.assetWorkbench.save") }}
          </button>
          <FDropdown :width="230">
            <template #trigger>
              <button
                type="button"
                class="maw-icon"
                :aria-label="t('studio.assetWorkbench.exportMenu')"
              >
                <FIcon name="Download" :size="14" aria-label="" />
              </button>
            </template>
            <button type="button" @click="exportSchema">
              <FIcon name="Braces" :size="13" aria-label="" />
              {{ t("studio.assetWorkbench.exportSchema") }}
            </button>
            <button type="button" :disabled="shotBusy" @click="exportRender">
              <FIcon
                :name="shotBusy ? 'Loader2' : 'Camera'"
                :size="13"
                aria-label=""
              />
              {{ t("package.exportRender") }}
            </button>
            <button
              v-if="asset.sourceKind.value === 'native'"
              type="button"
              :disabled="meshExportBusy"
              @click="exportMesh('white')"
            >
              <FIcon name="Box" :size="13" aria-label="" />
              {{ t("package.exportMeshWhite") }}
            </button>
            <button
              v-if="
                asset.sourceKind.value === 'native' &&
                effectiveRenderMode === 'refined'
              "
              type="button"
              :disabled="meshExportBusy"
              @click="exportMesh('textured')"
            >
              <FIcon name="Image" :size="13" aria-label="" />
              {{ t("package.exportMeshTextured") }}
            </button>
          </FDropdown>
          <FTooltip
            :text="
              editEnabled ? t('package.editLock') : t('package.editUnlock')
            "
            side="bottom"
          >
            <template #trigger>
              <button
                type="button"
                class="maw-icon"
                :class="{ active: editEnabled }"
                :aria-pressed="editEnabled"
                :aria-label="
                  editEnabled ? t('package.editLock') : t('package.editUnlock')
                "
                @click="toggleEdit"
              >
                <FIcon
                  :name="editEnabled ? 'LockOpen' : 'Lock'"
                  :size="14"
                  aria-label=""
                />
              </button>
            </template>
          </FTooltip>
          <!-- 移除当前资产：删除文档+导入的模型文件，回到空态可重新导入 -->
          <FTooltip
            :text="t('studio.assetWorkbench.removeAsset')"
            side="bottom"
          >
            <template #trigger>
              <button
                type="button"
                class="maw-icon"
                :disabled="removing"
                :aria-label="t('studio.assetWorkbench.removeAsset')"
                @click="onRemoveAsset"
              >
                <FIcon
                  :name="removing ? 'Loader2' : 'Trash2'"
                  :size="14"
                  aria-label=""
                />
              </button>
            </template>
          </FTooltip>
        </template>
      </div>
    </header>

    <Resizable direction="horizontal" class="maw-split">
      <ResizablePanel id="maw-tree" :default-size="17" :min-size="10">
        <AssetSourceTree
          ref="treeRef"
          :project="props.project"
          :active-asset="asset.sourceAssetPath.value"
          @open-asset="asset.openDraftAsset($event)"
          @open-package="onOpenPackage"
          @blend-imported="onBlendImported"
        />
      </ResizablePanel>
      <ResizableHandle
        orientation="horizontal"
        :label="t('studio.assetWorkbench.resizeTree')"
      />
      <ResizablePanel id="maw-center" :default-size="57" :min-size="30">
        <Resizable direction="vertical" class="maw-center">
          <ResizablePanel id="maw-viewport" :default-size="64" :min-size="22">
            <div class="maw-viewport">
              <template v-if="ui">
                <p v-if="ui.loadError" class="maw-load-error" role="alert">
                  {{ t(ui.loadError) }}
                </p>
                <SchemaPropertyViewport
                  v-else
                  ref="viewportRef"
                  v-bind="viewportBindings ?? {}"
                  @select="asset.selectUnit($event)"
                  @toggle-layer="ui.toggleGroup($event)"
                  @switch-lod="ui.switchLod($event)"
                  @select-tool="tool = $event"
                  @commit-transform="commitTransform"
                  @live-transform="
                    (id, value) =>
                      (liveTransform =
                        id && value ? { id, position: value.position } : null)
                  "
                  @delete-unit="asset.deleteUnit"
                />
                <PropertyEditorStatusBar
                  class="maw-status"
                  :grouping="ui.grouping"
                  :model-state="ui.modelState"
                  :selected-unit="ui.selectedUnit"
                />
              </template>
              <div v-else class="maw-empty">
                <div class="maw-empty-art" aria-hidden="true"></div>
                <FIcon name="Box" :size="30" aria-label="" />
                <strong>{{ t("studio.assetWorkbench.emptyTitle") }}</strong>
                <p>{{ t("studio.assetWorkbench.emptyHint") }}</p>
                <div class="maw-empty-actions">
                  <button
                    type="button"
                    class="maw-btn primary"
                    :disabled="asset.restoring.value"
                    @click="treeRef?.refresh()"
                  >
                    <FIcon name="FolderOpen" :size="13" aria-label="" />
                    {{ t("studio.assetWorkbench.emptyPickTree") }}
                  </button>
                  <button
                    type="button"
                    class="maw-btn"
                    @click="tgiSheetOpen = true"
                  >
                    <FIcon name="Hash" :size="13" aria-label="" />
                    {{ t("studio.assetWorkbench.tgiParse") }}
                  </button>
                </div>
                <FSpinner
                  v-if="asset.restoring.value"
                  size="sm"
                  :label="t('common.loading')"
                />
              </div>
            </div>
          </ResizablePanel>
          <ResizableHandle
            orientation="vertical"
            :label="t('studio.assetWorkbench.resizeBlueprint')"
          />
          <ResizablePanel id="maw-blueprint" :default-size="36" :min-size="14">
            <AssetBlueprintCanvas
              :source-kind="asset.sourceKind.value"
              :source-label="asset.sourceLabel.value"
              :unit-counts="unitCounts"
              :lot-size="ui?.lotSize ?? null"
              :menu="asset.menu"
              :saved-dirty="asset.savedDirty.value"
              :document-saved="Boolean(asset.schemaDocument.value)"
              @update:menu="asset.setMenuBinding($event)"
              @import-model="treeRef?.refresh()"
              @open-tgi="tgiSheetOpen = true"
            />
          </ResizablePanel>
        </Resizable>
      </ResizablePanel>
      <ResizableHandle
        orientation="horizontal"
        :label="t('studio.assetWorkbench.resizeRight')"
      />
      <ResizablePanel id="maw-right" :default-size="26" :min-size="15">
        <Resizable direction="vertical" class="maw-right">
          <ResizablePanel id="maw-right-tabs" :default-size="52" :min-size="18">
            <div class="maw-right-panel">
              <div class="maw-tabs" role="tablist">
                <button
                  v-for="tab in RIGHT_TABS"
                  :key="tab.id"
                  type="button"
                  role="tab"
                  class="maw-tab"
                  :class="{ active: rightTab === tab.id }"
                  :aria-selected="rightTab === tab.id"
                  @click="rightTab = tab.id"
                >
                  <FIcon :name="tab.icon" :size="12" aria-label="" />
                  <span>{{ t(tab.label) }}</span>
                </button>
              </div>
              <div class="maw-tab-body">
                <template v-if="ui">
                  <PropertyEditorOutliner
                    v-if="rightTab === 'tree'"
                    :grouping="ui.grouping"
                    :selected-id="ui.selectedId.value"
                    :hidden-units="ui.hiddenUnits"
                    :group-visibility="ui.groupVisibility"
                    :tree-ids="ui.propTreeIds"
                    @select="asset.selectUnit($event)"
                    @toggle-unit="ui.toggleUnit"
                    @toggle-group="ui.toggleGroup($event)"
                  />
                  <AssetComponentLibrary
                    v-else-if="rightTab === 'library'"
                    :edit-enabled="editEnabled"
                    :referenced-models="ui.referencedModels"
                    :prop-tree-ids="ui.propTreeIds"
                    :template-of="templateOf"
                    @place="placementPayload = $event"
                  />
                  <AssetLotPanel
                    v-else-if="rightTab === 'lot'"
                    :lot-size="ui.lotSize"
                    :edit-mode="ui.editMode"
                    @set-size="asset.setDraftLotSize($event)"
                  />
                  <AssetLotLibrary
                    v-else-if="rightTab === 'lotlib'"
                    :edit-enabled="editEnabled"
                    :edit-mode="ui.editMode"
                    @apply="onApplyLotPreset"
                  />
                  <AssetMaterialSlotsPanel
                    v-else-if="rightTab === 'slots'"
                    :project="props.project"
                    :binding="ui.materialSlots ?? {}"
                    :busy="asset.slotMaterialsComposing.value"
                    :disabled="ui.editMode !== 'draft'"
                    @bind="onBindMaterialSlots"
                  />
                  <div v-else class="maw-schema">
                    <MonacoEditor
                      :value="
                        asset.schemaJson.value ||
                        t('studio.assetWorkbench.schemaEmpty')
                      "
                      language="json"
                      fill
                    />
                  </div>
                </template>
                <p v-else class="maw-tab-empty">
                  {{ t("studio.assetWorkbench.noAssetRight") }}
                </p>
              </div>
            </div>
          </ResizablePanel>
          <ResizableHandle
            orientation="vertical"
            :label="t('studio.assetWorkbench.resizeInspector')"
          />
          <ResizablePanel
            id="maw-right-inspector"
            :default-size="48"
            :min-size="18"
          >
            <div class="maw-inspector">
              <template v-if="ui">
                <PropertyEditorInspector
                  :unit="ui.selectedUnit"
                  :live-transform="liveTransform"
                  :scale-editable="selectedScaleEditable"
                  :edit-enabled="editEnabled"
                  @update-transform="commitTransform"
                  @update-fields="commitFields"
                />
              </template>
              <p v-else class="maw-tab-empty">
                {{ t("studio.assetWorkbench.noAssetRight") }}
              </p>
            </div>
          </ResizablePanel>
        </Resizable>
      </ResizablePanel>
    </Resizable>

    <FSheet
      v-model:open="tgiSheetOpen"
      :label="t('studio.assetWorkbench.tgiParse')"
      width="min(460px, 94vw)"
    >
      <div class="tgi-body">
        <p class="tgi-hint">{{ t("studio.assetWorkbench.tgiHint") }}</p>
        <label class="tgi-field">
          <span>{{ t("studio.assetWorkbench.tgiPackage") }}</span>
          <div class="tgi-row">
            <input
              v-model="tgiPackagePath"
              type="text"
              :placeholder="t('studio.assetWorkbench.tgiPackagePlaceholder')"
              spellcheck="false"
            />
            <button
              type="button"
              class="maw-icon"
              :aria-label="t('studio.assetWorkbench.tgiBrowse')"
              :title="t('studio.assetWorkbench.tgiBrowse')"
              @click="browsePackage"
            >
              <FIcon name="FolderOpen" :size="13" aria-label="" />
            </button>
          </div>
        </label>
        <label class="tgi-field">
          <span>{{ t("studio.assetWorkbench.tgiValue") }}</span>
          <input
            v-model="tgiText"
            type="text"
            class="tgi-mono"
            :placeholder="t('studio.assetWorkbench.tgiPlaceholder')"
            spellcheck="false"
            @keydown.enter="submitTgi"
          />
        </label>
        <template v-if="packageCandidates.length">
          <p class="tgi-candidates-title">
            {{
              t("studio.assetWorkbench.tgiCandidates", {
                count: packageCandidates.length,
              })
            }}
          </p>
          <button
            v-for="(candidate, index) in packageCandidates"
            :key="index"
            type="button"
            class="tgi-candidate"
            :class="{
              active:
                tgiText &&
                parseTgiText(tgiText)?.instance === candidate.instance,
            }"
            @click="openCandidate(candidate)"
          >
            <FIcon name="FilePen" :size="13" aria-label="" />
            <span class="tgi-mono">
              {{ hex32(candidate.typeId) }} : {{ hex32(candidate.group) }} :
              {{ hex32(candidate.instance) }}
            </span>
          </button>
        </template>
        <button
          type="button"
          class="maw-btn primary tgi-submit"
          :disabled="tgiBusy || !tgiPackagePath || !parseTgiText(tgiText)"
          @click="submitTgi"
        >
          <FIcon
            :name="tgiBusy ? 'Loader2' : 'PackageSearch'"
            :size="13"
            aria-label=""
          />
          {{ t("studio.assetWorkbench.tgiSubmit") }}
        </button>
      </div>
    </FSheet>
  </section>
</template>

<style scoped>
.maw {
  background: var(--surface);
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  min-width: 0;
}
.maw-toolbar {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  flex-wrap: wrap;
  gap: 8px;
  padding: 7px 10px;
}
.maw-title {
  align-items: center;
  display: flex;
  flex: 1;
  gap: 8px;
  min-width: 0;
}
.maw-title strong {
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.maw-badge {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--muted-foreground);
  display: inline-flex;
  flex: none;
  font-size: 10px;
  gap: 4px;
  padding: 2px 8px;
}
.maw-badge[data-kind="native"] {
  color: var(--primary);
}
.maw-dirty {
  color: var(--warning);
  flex: none;
  font-size: 10.5px;
}
.maw-actions {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}
.maw-gap {
  inline-size: 6px;
}
.maw-btn,
.maw-icon {
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
  min-height: 27px;
  padding: 0 9px;
  white-space: nowrap;
}
.maw-icon {
  justify-content: center;
  min-width: 27px;
  padding: 0;
}
.maw-btn:hover:not(:disabled),
.maw-icon:hover:not(:disabled) {
  background: var(--surface-hover);
}
.maw-btn:disabled,
.maw-icon:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.maw-btn.primary {
  background: var(--primary);
  border-color: var(--primary);
  color: #fff;
}
.maw-btn.primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 85%, #fff);
}
.maw-icon.active {
  border-color: var(--primary);
  color: var(--primary);
}
.maw-seg {
  display: inline-flex;
}
.maw-seg button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  min-height: 25px;
  padding: 2px 9px;
}
.maw-seg button:first-child {
  border-end-end-radius: 0;
  border-start-end-radius: 0;
}
.maw-seg button:last-child {
  border-end-start-radius: 0;
  border-start-start-radius: 0;
  margin-inline-start: -1px;
}
.maw-seg button.active {
  color: var(--foreground);
}
.maw-seg button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.maw-daynight {
  align-items: center;
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 11px;
  gap: 6px;
  white-space: nowrap;
}
.maw-daynight input[type="range"] {
  width: 96px;
}
.maw-daynight-value {
  font-variant-numeric: tabular-nums;
  min-width: 34px;
  text-align: right;
}
.maw-switch {
  align-items: center;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font-size: 11px;
  gap: 6px;
  white-space: nowrap;
}
.maw-split {
  border-inline: 1px solid var(--border);
  flex: 1;
  min-height: 0;
}
.maw-center,
.maw-right {
  height: 100%;
  min-height: 0;
}
.maw-viewport {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  position: relative;
}
.maw-viewport :deep(.viewport-pane) {
  flex: 1;
  min-height: 0;
}
.maw-load-error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  padding: 22px 14px;
  text-align: center;
}
.maw-status {
  border-top: 1px solid var(--border);
  flex: none;
}
.maw-empty {
  align-items: center;
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 9px;
  justify-content: center;
  overflow: hidden;
  position: relative;
  text-align: center;
}
.maw-empty-art {
  background-image:
    linear-gradient(
      color-mix(in srgb, var(--primary) 18%, transparent) 1px,
      transparent 1px
    ),
    linear-gradient(
      90deg,
      color-mix(in srgb, var(--primary) 18%, transparent) 1px,
      transparent 1px
    );
  background-size: 30px 30px;
  inset: 0;
  opacity: 0.6;
  position: absolute;
  transform: perspective(460px) rotateX(56deg) scale(1.6);
  transform-origin: center bottom;
}
.maw-empty > :not(.maw-empty-art) {
  position: relative;
}
.maw-empty strong {
  color: var(--foreground);
  font-size: 14px;
}
.maw-empty p {
  color: var(--muted-foreground);
  font-size: 11.5px;
  margin: 0;
  max-width: 340px;
}
.maw-empty-actions {
  display: flex;
  gap: 7px;
  margin-top: 4px;
}
.maw-right-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.maw-tabs {
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  overflow-x: auto;
}
.maw-tab {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  flex: 1;
  font: inherit;
  font-size: 10.5px;
  gap: 4px;
  justify-content: center;
  min-height: 29px;
  padding: 0 6px;
  white-space: nowrap;
}
.maw-tab:hover {
  color: var(--foreground);
}
.maw-tab.active {
  border-bottom-color: var(--primary);
  color: var(--foreground);
  font-weight: 600;
}
.maw-tab-body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.maw-tab-body > * {
  height: 100%;
}
.maw-schema {
  height: 100%;
  min-height: 0;
}
.maw-tab-empty {
  color: var(--muted-foreground);
  font-size: 11.5px;
  margin: 0;
  padding: 20px 10px;
  text-align: center;
}
.maw-inspector {
  height: 100%;
  min-height: 0;
  overflow: auto;
}
.tgi-body {
  display: flex;
  flex-direction: column;
  gap: 11px;
  padding: 12px 14px;
}
.tgi-hint {
  color: var(--muted-foreground);
  font-size: 11.5px;
  line-height: 1.55;
  margin: 0;
}
.tgi-field {
  color: var(--muted-foreground);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 5px;
}
.tgi-field input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  min-height: 29px;
  padding: 0 9px;
}
.tgi-row {
  display: flex;
  gap: 5px;
}
.tgi-row input {
  flex: 1;
  min-width: 0;
}
.tgi-mono {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
}
.tgi-candidates-title {
  color: var(--muted-foreground);
  font-size: 10.5px;
  letter-spacing: 0.05em;
  margin: 0;
  text-transform: uppercase;
}
.tgi-candidate {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  gap: 7px;
  min-height: 29px;
  padding: 0 9px;
  text-align: start;
}
.tgi-candidate:hover {
  border-color: var(--primary);
}
.tgi-candidate.active {
  border-color: var(--primary);
  color: var(--primary);
}
.tgi-submit {
  justify-content: center;
  margin-top: 2px;
}
</style>
