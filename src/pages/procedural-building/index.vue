<script setup lang="ts">
import { computed, onMounted, reactive, ref, shallowRef } from "vue";
import { watchDebounced } from "@vueuse/core";
import { useI18n } from "vue-i18n";
import FButton from "@/components/ui/FButton.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FSheet from "@/components/ui/FSheet.vue";
import PropertyEditor from "@/pages/packages/components/property-editor/PropertyEditor.vue";
import LotTexturePanel from "./LotTexturePanel.vue";
import PropertyMaterialInput from "./PropertyMaterialInput.vue";
import MassingPanel from "./MassingPanel.vue";
import { usePropertyMaterial } from "./usePropertyMaterial";
import { command } from "@/api/tauri";
import { tauriApi } from "@/api";
import { useToast } from "@/composables/useToast";
import { generateBuildingParts } from "@/lib/procedural-building/generator";
import { buildModelPayload } from "@/lib/procedural-building/meshBuilder";
import {
  DEFAULT_PARAMETERS,
  buildingFootprint,
  createProceduralBuildingDocument,
  type ProceduralBuildingParameters,
} from "@/lib/procedural-building/schema";
import {
  massPreset,
  normalizeBlockMass,
} from "@/lib/procedural-building/massing";
import { useLotGround, type LotSelection } from "./lotGround";
import type { LotEditorSession, LotModelPayload, Tgi } from "@/api/tauri";

/** 程序化建筑 demo：Property Editor 全屏承载合成会话；生成器面板收在
 * 右侧 FSheet（开关集成在编辑器工具栏 toolbar-extra 插槽），解锁后从
 * 物料面板拖入 prop/灯光/特效/刷怪器完成场景装配。 */

const { t } = useI18n();
const toast = useToast();
const {
  input: propertyTgi,
  loading: materialLoading,
  errorKey: materialError,
  resolved: propertyMaterial,
  resolve: resolveMaterial,
} = usePropertyMaterial();
const editorOpen = ref(true);
// 生成器面板默认收起：进入页面先看到完整视口，需要时从工具栏展开
const generatorOpen = ref(false);
const zeroTgi: Tgi = { typeId: 0, group: 0, instance: 0 };

const parameters = reactive<ProceduralBuildingParameters>({
  ...DEFAULT_PARAMETERS,
  blockMass: massPreset("single"),
});
const seed = ref(20261018);
const synthetic = shallowRef<{
  session: LotEditorSession;
  payload: LotModelPayload;
} | null>(null);
const generating = ref(false);
const partsCount = ref(0);
const peRef = ref<{
  getSchema: () => Record<string, unknown> | null;
  restoreSchema: (document: Record<string, unknown>) => void;
  getRenderMode: () => "default" | "refined";
  groundPointAt: (x: number, y: number) => [number, number, number] | null;
} | null>(null);

/** Lot 地面遮罩（raster 工作台「真实引用」口径的 LotMask）：绑进合成
 * 会话的 lotMaskPng，由视口既有地面合成器按 LotColor 渲染——不是
 * albedo 贴图。 */
const lotGround = useLotGround();
const lotSession = computed(() => lotGround.selected.value?.session ?? null);
const lotMask = computed(() => {
  const selected = lotGround.selected.value;
  return selected
    ? { dataUrl: selected.dataUrl, label: selected.selection.label }
    : null;
});
const footprintLabel = computed(() => {
  const { width, depth } = buildingFootprint(parameters);
  return t("proceduralBuilding.massSummary", {
    width: width.toFixed(1),
    depth: depth.toFixed(1),
    height: Math.max(
      0,
      ...(parameters.blockMass?.blocks ?? []).map(
        (b) => b.center[2] + b.size[2] / 2,
      ),
    ).toFixed(1),
    parts: partsCount.value,
  });
});

function makeSession(): LotEditorSession {
  const { width, depth } = buildingFootprint(parameters);
  const source = lotSession.value;
  return {
    tgi: { ...zeroTgi },
    assetName: t("proceduralBuilding.sessionTitle", { seed: seed.value }),
    modelAvailable: true,
    modelKey: null,
    modelLods: [null, null, null, null],
    lotSize: source?.lotSize ?? [width, depth],
    lotTilePeriod: source?.lotTilePeriod ?? null,
    lotPlacement: source?.lotPlacement ?? null,
    lotMaskPng: source?.lotMaskPng ?? null,
    lotMaskRawRgba: source?.lotMaskRawRgba ?? null,
    lotAlbedoPng: source?.lotAlbedoPng ?? null,
    lotSurfacePng: source?.lotSurfacePng ?? null,
    lotNormalAtlasPng: source?.lotNormalAtlasPng ?? null,
    lotColors: source?.lotColors ?? [
      [0, 0, 0, 0],
      [255, 0, 0, 0],
      [0, 255, 0, 0],
      [0, 0, 255, 0],
    ],
    lotColorsAuthored: source?.lotColorsAuthored ?? [
      false,
      false,
      false,
      false,
    ],
    lotBorderColors: source?.lotBorderColors ?? [
      [156, 156, 156],
      [156, 156, 156],
      [156, 156, 156],
      [156, 156, 156],
    ],
    lotBorderWidths: source?.lotBorderWidths ?? [0, 0, 0, 0],
    lotColorHeights: source?.lotColorHeights ?? [0, 0, 0, 0],
    lotBorderHeights: source?.lotBorderHeights ?? [0, 0, 0, 0],
    lotBorderPatternIndices: source?.lotBorderPatternIndices ?? [0, 0, 0, 0],
    lotBaseTile: source?.lotBaseTile ?? 8,
    lotOverlayBoxOffset: source?.lotOverlayBoxOffset ?? null,
    lotModelBBoxCenter: source?.lotModelBBoxCenter ?? null,
    units: [],
    decalTextures: [],
    pathPairs: [],
    diagnostics: [],
  };
}

let buildToken = 0;
async function regenerate() {
  const token = ++buildToken;
  generating.value = true;
  try {
    const generated = generateBuildingParts({ ...parameters }, seed.value);
    const parts = lotMask.value
      ? generated.filter((part) => part.id !== "slab")
      : generated;
    if (token !== buildToken) return;
    partsCount.value = parts.length;
    const payload = await buildModelPayload({
      parts,
      buildingMaterial: propertyMaterial.value?.source,
      windowSpacing: parameters.windowSpacing ?? 0,
    });
    if (token !== buildToken) return;
    // 新 payload 对象触发视口重建；session 每代换新，但编辑层不重置——
    // 用户拖放的 unit 跨参数调整存活。
    synthetic.value = { session: makeSession(), payload };
  } catch (error) {
    if (token === buildToken)
      toast.error(t("proceduralBuilding.generateFailed"));
  } finally {
    if (token === buildToken) generating.value = false;
  }
}

watchDebounced(
  [() => ({ ...parameters }), seed, propertyMaterial, lotSession],
  () => void regenerate(),
  { debounce: 250, deep: true },
);
onMounted(() => void regenerate());

// ---- Lot 地面遮罩（PE「Lot 地面遮罩」页签点选）----
async function bindLotMask(payload: LotSelection) {
  try {
    if (await lotGround.select(payload))
      toast.success(
        t("proceduralBuilding.lotMaskBound", { label: payload.label }),
      );
  } catch {
    toast.error(t("proceduralBuilding.bindFailed"));
  }
}
function unbindLotMask() {
  lotGround.clear();
}
function dropLotMask(payload: LotSelection, x: number, y: number) {
  if (document.elementFromPoint(x, y)?.closest(".lot-panel, .panel-body"))
    return;
  if (peRef.value?.groundPointAt(x, y)) void bindLotMask(payload);
}

// ---- 导出（生成器参数 + 贴图绑定 + Property Editor 的 lot 资产文档）----
const exporting = ref(false);
async function exportDocument() {
  if (exporting.value) return;
  exporting.value = true;
  try {
    const document = createProceduralBuildingDocument(
      {
        ...parameters,
        blockMass: parameters.blockMass
          ? normalizeBlockMass(parameters.blockMass)
          : undefined,
      },
      seed.value,
      {},
      peRef.value?.getSchema() ?? null,
      propertyMaterial.value?.binding ?? null,
    );
    const binary = JSON.stringify(
      { ...document, lotGround: lotGround.selected.value?.selection ?? null },
      null,
      2,
    );
    let payload = "";
    for (const byte of new TextEncoder().encode(binary))
      payload += String.fromCharCode(byte);
    const path = await tauriApi.packages.saveFile(
      `procedural-building-${seed.value}.building.json`,
      "json",
    );
    if (!path) return;
    await command("write_export_file", {
      request: { path, dataBase64: btoa(payload) },
    });
    toast.success(t("proceduralBuilding.exportOk", { path }));
  } catch {
    toast.error(t("proceduralBuilding.exportFailed"));
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <section class="procedural-page">
    <PropertyEditor
      ref="peRef"
      v-model:open="editorOpen"
      :embedded="true"
      :empty="!synthetic"
      :package-id="0"
      :tgi="zeroTgi"
      :synthetic="synthetic"
    >
      <template #toolbar-extra>
        <button
          type="button"
          class="panel-toggle"
          :class="{ active: generatorOpen }"
          :aria-pressed="generatorOpen"
          :aria-label="$t('proceduralBuilding.generatorPanel')"
          :title="$t('proceduralBuilding.generatorPanel')"
          @click="generatorOpen = !generatorOpen"
        >
          <FIcon name="SlidersHorizontal" :size="15" aria-label="" />
        </button>
      </template>
      <template #materials-lots>
        <!-- Lot 页签：真实引用的 Lot 地面遮罩，点选加载进地面合成器 -->
        <LotTexturePanel @pick="bindLotMask" @drop="dropLotMask" />
      </template>
    </PropertyEditor>

    <div v-if="!editorOpen" class="editor-reopen">
      <FButton variant="secondary" @click="editorOpen = true">
        <FIcon name="PanelTop" :size="14" aria-label="" />
        {{ $t("proceduralBuilding.reopenEditor") }}
      </FButton>
    </div>

    <!-- 生成器面板：右侧 sheet，复用物料面板的同款宽档 -->
    <FSheet
      v-model:open="generatorOpen"
      :label="$t('proceduralBuilding.generatorPanel')"
      width="min(440px, 94vw)"
    >
      <div class="panel-body">
        <header class="panel-head">
          <div class="panel-title">
            <strong>{{ $t("proceduralBuilding.sidebarTitle") }}</strong>
            <span class="panel-summary">{{ footprintLabel }}</span>
          </div>
          <FSpinner
            v-if="generating"
            size="sm"
            :label="$t('proceduralBuilding.generating')"
          />
        </header>

        <PropertyMaterialInput
          v-model="propertyTgi"
          :loading="materialLoading"
          :error-key="materialError"
          :loaded-name="propertyMaterial?.name"
          :lod="propertyMaterial?.binding.lod"
          @resolve="resolveMaterial"
        />

        <MassingPanel
          v-if="parameters.blockMass"
          v-model="parameters.blockMass"
        />

        <div class="panel-section window-spacing-section">
          <h3>{{ $t("proceduralBuilding.windowSpacing") }}</h3>
          <p class="panel-hint">
            {{ $t("proceduralBuilding.windowSpacingHint") }}
          </p>
          <label class="window-spacing-control">
            <span>{{ (parameters.windowSpacing ?? 0).toFixed(1) }} m</span>
            <input
              v-model.number="parameters.windowSpacing"
              type="range"
              min="0"
              max="12"
              step="0.5"
              :aria-label="$t('proceduralBuilding.windowSpacing')"
            />
          </label>
        </div>

        <div class="panel-section">
          <h3>{{ $t("proceduralBuilding.lotGround") }}</h3>
          <div class="slot-row">
            <img
              v-if="lotMask"
              class="slot-chip slot-chip-texture"
              :src="lotMask.dataUrl"
              alt=""
            />
            <span v-else class="slot-chip slot-chip-empty" />
            <span class="slot-name">{{
              $t("proceduralBuilding.lotGround")
            }}</span>
            <span class="slot-binding" :title="lotMask?.label">
              {{ lotMask?.label ?? $t("proceduralBuilding.lotUnbound") }}
            </span>
            <span class="slot-row-spacer" />
            <button
              v-if="lotMask"
              type="button"
              class="mini-btn"
              :aria-label="$t('proceduralBuilding.clearTexture')"
              @click="unbindLotMask"
            >
              <FIcon name="Eraser" :size="13" aria-label="" />
            </button>
          </div>
          <p class="panel-hint">{{ $t("proceduralBuilding.lotGroundHint") }}</p>
        </div>

        <div class="panel-section panel-actions">
          <FButton :disabled="exporting" @click="exportDocument">
            <FIcon name="Download" :size="14" aria-label="" />
            {{ $t("proceduralBuilding.exportDocument") }}
          </FButton>
          <p class="panel-hint">{{ $t("proceduralBuilding.unlockHint") }}</p>
        </div>
      </div>
    </FSheet>
  </section>
</template>

<style scoped>
.procedural-page {
  display: flex;
  /* 抵消 content-frame 的镀铬内边距：全幅贴边（顶 36px，水平 clamp）；
   * 高度补回负 margin 吃掉的顶部镀铬，贴住页脚上缘 */
  height: calc(100% + 36px);
  margin: -36px calc(-1 * clamp(20px, 4vw, 56px)) 0;
  min-height: 0;
  position: relative;
}
@media (max-width: 720px) {
  .procedural-page {
    height: calc(100% + 24px);
    margin: -24px -16px 0;
  }
}
.procedural-page :deep(.f-sheet-embedded) {
  display: flex;
  flex: 1;
  min-height: 0;
  width: 100%;
}
.procedural-page :deep(.editor-root) {
  border: 0;
  border-radius: 0;
  flex: 1;
  height: auto;
  min-height: 0;
  overflow: hidden;
  width: 100%;
}
/* 工具栏扩展位按钮：对齐 PE 头部 editor-close 的观感（scoped 隔离，
 * 复用其尺寸/描边/激活态三件套） */
.panel-toggle {
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
.panel-toggle:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.panel-toggle.active {
  border-color: var(--brand);
  color: var(--brand);
}

.editor-reopen {
  align-items: center;
  display: flex;
  inset: 0;
  justify-content: center;
  pointer-events: none;
  position: absolute;
}
.editor-reopen :deep(.f-button) {
  pointer-events: auto;
}

.panel-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 14px;
}
.panel-head {
  align-items: flex-start;
  display: flex;
  gap: 8px;
  justify-content: space-between;
}
.panel-title {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.panel-summary {
  font-size: 12px;
  opacity: 0.65;
}
.panel-section {
  border-top: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 12px;
}
.panel-section h3 {
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.04em;
  margin: 0;
  opacity: 0.75;
  text-transform: uppercase;
}
.param {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: 1fr 72px;
}
.param span {
  font-size: 12px;
}
.param input {
  grid-column: 1;
  width: 100%;
}
.param em {
  font-size: 12px;
  font-style: normal;
  opacity: 0.7;
  text-align: right;
}
.segment {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.segment > span {
  font-size: 12px;
}
.segment > div {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.segment button {
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: inherit;
  cursor: pointer;
  font-size: 12px;
  padding: 3px 8px;
}
.segment button.active {
  background: var(--primary);
  border-color: var(--primary);
  color: #fff;
}
.seed-row {
  display: flex;
  gap: 6px;
}
.seed-input {
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: inherit;
  flex: 1;
  font-size: 12px;
  min-width: 0;
  padding: 4px 8px;
}
.mini-btn {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: inherit;
  cursor: pointer;
  display: inline-flex;
  height: 26px;
  justify-content: center;
  width: 26px;
}
.mini-btn:hover {
  border-color: var(--primary);
  color: var(--primary);
}
.slot-row {
  align-items: center;
  display: grid;
  gap: 6px;
  grid-template-columns: 16px auto 1fr auto auto;
}
.slot-chip {
  border: 1px solid rgba(255, 255, 255, 0.25);
  border-radius: 3px;
  display: inline-block;
  height: 16px;
  object-fit: cover;
  width: 16px;
}
.slot-chip-empty {
  background: var(--surface-hover, rgba(255, 255, 255, 0.08));
}
.slot-row-spacer {
  flex: 1;
}
.slot-name {
  font-size: 12px;
}
.slot-binding {
  font-size: 11px;
  overflow: hidden;
  opacity: 0.65;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.panel-actions {
  margin-top: auto;
}
.panel-hint {
  font-size: 11px;
  line-height: 1.5;
  opacity: 0.6;
}
</style>
