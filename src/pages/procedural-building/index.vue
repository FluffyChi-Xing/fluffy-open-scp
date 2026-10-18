<script setup lang="ts">
import { computed, onMounted, reactive, ref, shallowRef } from "vue";
import { watchDebounced } from "@vueuse/core";
import { useI18n } from "vue-i18n";
import FButton from "@/components/ui/FButton.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FSheet from "@/components/ui/FSheet.vue";
import FTooltip from "@/components/ui/FTooltip.vue";
import PropertyEditor from "@/pages/packages/components/property-editor/PropertyEditor.vue";
import LotTexturePanel from "./LotTexturePanel.vue";
import TexturePickerSheet from "./TexturePickerSheet.vue";
import { command } from "@/api/tauri";
import { tauriApi } from "@/api";
import { useToast } from "@/composables/useToast";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { generateBuildingParts } from "@/lib/procedural-building/generator";
import { buildModelPayload } from "@/lib/procedural-building/meshBuilder";
import {
  DEFAULT_PARAMETERS,
  MATERIAL_SLOTS,
  buildingFootprint,
  createProceduralBuildingDocument,
  type MaterialSlotId,
  type ProceduralBuildingParameters,
  type SlotBindingMap,
} from "@/lib/procedural-building/schema";
import { loadRasterPng } from "@/lib/procedural-building/textures";
import type { LotEditorSession, LotModelPayload, Tgi } from "@/api/tauri";

/** 程序化建筑 demo：Property Editor 全屏承载合成会话；生成器面板收在
 * 右侧 FSheet（开关集成在编辑器工具栏 toolbar-extra 插槽），解锁后从
 * 物料面板拖入 prop/灯光/特效/刷怪器完成场景装配。 */

const { t } = useI18n();
const toast = useToast();
const packagesStore = useGamePackagesStore();
const editorOpen = ref(true);
// 生成器面板默认收起：进入页面先看到完整视口，需要时从工具栏展开
const generatorOpen = ref(false);
const zeroTgi: Tgi = { typeId: 0, group: 0, instance: 0 };

const parameters = reactive<ProceduralBuildingParameters>({ ...DEFAULT_PARAMETERS });
const seed = ref(20261018);
const bindings = reactive<SlotBindingMap>({});
/** 槽 → 已解码的贴图 PNG 字节（合成 payload 的 baseColor）。 */
const slotTextures = shallowRef(new Map<MaterialSlotId, Uint8Array<ArrayBuffer>>());
/** 槽 → 缩略图 data URL（面板 chip 预览）。 */
const slotThumbs = shallowRef(new Map<MaterialSlotId, string>());
const slotLabels = shallowRef(new Map<MaterialSlotId, string>());
const synthetic = shallowRef<{
  session: LotEditorSession;
  payload: LotModelPayload;
} | null>(null);
const generating = ref(false);
const partsCount = ref(0);
const pickerOpen = ref(false);
const pickingSlot = ref<MaterialSlotId | null>(null);
const peRef = ref<{
  getSchema: () => Record<string, unknown> | null;
  restoreSchema: (document: Record<string, unknown>) => void;
  getRenderMode: () => "default" | "refined";
} | null>(null);

/** Lot 地面遮罩（raster 工作台「真实引用」口径的 LotMask）：绑进合成
 * 会话的 lotMaskPng，由视口既有地面合成器按 LotColor 渲染——不是
 * albedo 贴图。 */
const lotMask = shallowRef<{ dataUrl: string; label: string } | null>(null);

const footprintLabel = computed(() => {
  const { width, depth } = buildingFootprint(parameters);
  return t("proceduralBuilding.lotSummary", {
    width,
    depth,
    floors: parameters.floors + 1,
    parts: partsCount.value,
  });
});

function makeSession(): LotEditorSession {
  const { width, depth } = buildingFootprint(parameters);
  return {
    tgi: { ...zeroTgi },
    assetName: t("proceduralBuilding.sessionTitle", { seed: seed.value }),
    modelAvailable: true,
    modelKey: null,
    modelLods: [null, null, null, null],
    lotSize: [width, depth],
    lotTilePeriod: null,
    lotPlacement: null,
    lotMaskPng: lotMask.value?.dataUrl ?? null,
    lotMaskRawRgba: null,
    lotAlbedoPng: null,
    lotSurfacePng: null,
    lotNormalAtlasPng: null,
    lotColors: [
      [0, 0, 0, 0],
      [255, 0, 0, 0],
      [0, 255, 0, 0],
      [0, 0, 255, 0],
    ],
    lotColorsAuthored: [false, false, false, false],
    lotBorderColors: [
      [156, 156, 156],
      [156, 156, 156],
      [156, 156, 156],
      [156, 156, 156],
    ],
    lotBorderWidths: [0, 0, 0, 0],
    lotColorHeights: [0, 0, 0, 0],
    lotBorderHeights: [0, 0, 0, 0],
    lotBorderPatternIndices: [0, 0, 0, 0],
    lotBaseTile: 8,
    lotOverlayBoxOffset: null,
    lotModelBBoxCenter: null,
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
      ? generated.filter(part => part.id !== "slab")
      : generated;
    if (token !== buildToken) return;
    partsCount.value = parts.length;
    const payload = await buildModelPayload({
      parts,
      textures: slotTextures.value,
      textureLabels: slotLabels.value,
    });
    if (token !== buildToken) return;
    // 新 payload 对象触发视口重建；session 每代换新，但编辑层不重置——
    // 用户拖放的 unit 跨参数调整存活。
    synthetic.value = { session: makeSession(), payload };
  } catch (error) {
    toast.error(error instanceof Error ? error.message : t("proceduralBuilding.generateFailed"));
  } finally {
    if (token === buildToken) generating.value = false;
  }
}

watchDebounced(
  [() => ({ ...parameters }), seed, slotTextures, lotMask],
  () => void regenerate(),
  { debounce: 150, deep: true },
);
onMounted(() => void regenerate());

// ---- 材质槽贴图绑定 ----
function openPicker(slot: MaterialSlotId) {
  pickingSlot.value = slot;
  pickerOpen.value = true;
}
/** 绑定槽 → 游戏 raster（解码失败回滚绑定）。 */
async function bindSlot(
  slot: MaterialSlotId,
  payload: { packageId: number; tgi: Tgi; label: string },
) {
  try {
    const png = await loadRasterPng(payload.packageId, payload.tgi);
    if (!png) {
      toast.error(t("proceduralBuilding.pickerNotDecodable"));
      return;
    }
    bindings[slot] = {
      slot,
      packageId: payload.packageId,
      packagePath: storePackagePath(payload.packageId),
      tgi: {
        type: payload.tgi.typeId,
        group: payload.tgi.group,
        instance: payload.tgi.instance,
      },
      label: payload.label,
    };
    const next = new Map(slotTextures.value);
    next.set(slot, png.bytes);
    slotTextures.value = next;
    const nextThumbs = new Map(slotThumbs.value);
    nextThumbs.set(slot, png.dataUrl);
    slotThumbs.value = nextThumbs;
    const nextLabels = new Map(slotLabels.value);
    nextLabels.set(slot, payload.label);
    slotLabels.value = nextLabels;
    toast.success(
      t("proceduralBuilding.slotBound", {
        slot: t(`proceduralBuilding.slot_${slot}`),
      }),
    );
    if (peRef.value?.getRenderMode?.() === "default")
      toast.info(t("proceduralBuilding.refinedHint"));
  } catch {
    toast.error(t("proceduralBuilding.bindFailed"));
  }
}
async function onPick(payload: {
  packageId: number;
  tgi: Tgi;
  label: string;
}) {
  const slot = pickingSlot.value;
  if (!slot) return;
  await bindSlot(slot, payload);
}
function unbindSlot(slot: MaterialSlotId) {
  delete bindings[slot];
  const next = new Map(slotTextures.value);
  next.delete(slot);
  slotTextures.value = next;
  const nextThumbs = new Map(slotThumbs.value);
  nextThumbs.delete(slot);
  slotThumbs.value = nextThumbs;
  const nextLabels = new Map(slotLabels.value);
  nextLabels.delete(slot);
  slotLabels.value = nextLabels;
}
/** 已打开包的路径（绑定持久化用；找不到为 undefined）。 */
function storePackagePath(packageId: number): string | undefined {
  return packagesStore.opened.find(
    entry => entry.package.packageId === packageId,
  )?.package.path;
}

// ---- Lot 地面遮罩（PE「Lot 地面遮罩」页签点选）----
async function bindLotMask(payload: {
  packageId: number;
  tgi: Tgi;
  label: string;
}) {
  try {
    const png = await loadRasterPng(payload.packageId, payload.tgi);
    if (!png) {
      toast.error(t("proceduralBuilding.pickerNotDecodable"));
      return;
    }
    lotMask.value = { dataUrl: png.dataUrl, label: payload.label };
    toast.success(t("proceduralBuilding.lotMaskBound", { label: payload.label }));
    if (peRef.value?.getRenderMode?.() === "default")
      toast.info(t("proceduralBuilding.refinedHint"));
  } catch {
    toast.error(t("proceduralBuilding.bindFailed"));
  }
}
function unbindLotMask() {
  lotMask.value = null;
}

function reseed() {
  seed.value = Math.floor(Math.random() * 0xffffffff) >>> 0;
}

// ---- 导出（生成器参数 + 贴图绑定 + Property Editor 的 lot 资产文档）----
const exporting = ref(false);
async function exportDocument() {
  if (exporting.value) return;
  exporting.value = true;
  try {
    const document = createProceduralBuildingDocument(
      { ...parameters },
      seed.value,
      bindings,
      peRef.value?.getSchema() ?? null,
    );
    const binary = JSON.stringify(document, null, 2);
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
        <LotTexturePanel @pick="bindLotMask" />
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
      width="min(360px, 92vw)"
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

        <div class="panel-section">
          <h3>{{ $t("proceduralBuilding.sectionMassing") }}</h3>
          <label class="param">
            <span>{{ $t("proceduralBuilding.baysX") }}</span>
            <input
              v-model.number="parameters.baysX"
              type="range"
              min="1"
              max="12"
              step="1"
              :aria-label="$t('proceduralBuilding.baysX')"
            />
            <em>{{ parameters.baysX }}</em>
          </label>
          <label class="param">
            <span>{{ $t("proceduralBuilding.baysY") }}</span>
            <input
              v-model.number="parameters.baysY"
              type="range"
              min="1"
              max="12"
              step="1"
              :aria-label="$t('proceduralBuilding.baysY')"
            />
            <em>{{ parameters.baysY }}</em>
          </label>
          <label class="param">
            <span>{{ $t("proceduralBuilding.floors") }}</span>
            <input
              v-model.number="parameters.floors"
              type="range"
              min="1"
              max="40"
              step="1"
              :aria-label="$t('proceduralBuilding.floors')"
            />
            <em>{{ parameters.floors }}</em>
          </label>
          <label class="param">
            <span>{{ $t("proceduralBuilding.groundFloorHeight") }}</span>
            <input
              v-model.number="parameters.groundFloorHeight"
              type="range"
              min="3"
              max="8"
              step="0.1"
              :aria-label="$t('proceduralBuilding.groundFloorHeight')"
            />
            <em>{{ parameters.groundFloorHeight.toFixed(1) }} m</em>
          </label>
          <label class="param">
            <span>{{ $t("proceduralBuilding.floorHeight") }}</span>
            <input
              v-model.number="parameters.floorHeight"
              type="range"
              min="2.4"
              max="8"
              step="0.1"
              :aria-label="$t('proceduralBuilding.floorHeight')"
            />
            <em>{{ parameters.floorHeight.toFixed(1) }} m</em>
          </label>
        </div>

        <div class="panel-section">
          <h3>{{ $t("proceduralBuilding.sectionStyle") }}</h3>
          <div class="segment">
            <span>{{ $t("proceduralBuilding.roofStyle") }}</span>
            <div role="group" :aria-label="$t('proceduralBuilding.roofStyle')">
              <button
                v-for="mode in ['mansard', 'flat']"
                :key="mode"
                type="button"
                :class="{ active: parameters.roofStyle === mode }"
                :aria-pressed="parameters.roofStyle === mode"
                @click="parameters.roofStyle = mode as 'mansard' | 'flat'"
              >
                {{ $t(`proceduralBuilding.roof_${mode}`) }}
              </button>
            </div>
          </div>
          <div class="segment">
            <span>{{ $t("proceduralBuilding.balconies") }}</span>
            <div role="group" :aria-label="$t('proceduralBuilding.balconies')">
              <button
                v-for="mode in ['none', 'featured', 'all']"
                :key="mode"
                type="button"
                :class="{ active: parameters.balconies === mode }"
                :aria-pressed="parameters.balconies === mode"
                @click="parameters.balconies = mode as 'none' | 'featured' | 'all'"
              >
                {{ $t(`proceduralBuilding.balconies_${mode}`) }}
              </button>
            </div>
          </div>
          <div class="segment">
            <span>{{ $t("proceduralBuilding.detail") }}</span>
            <div role="group" :aria-label="$t('proceduralBuilding.detail')">
              <button
                v-for="mode in ['off', 'uniform', 'alternating', 'random']"
                :key="mode"
                type="button"
                :class="{ active: parameters.detail === mode }"
                :aria-pressed="parameters.detail === mode"
                @click="parameters.detail = mode as typeof parameters.detail"
              >
                {{ $t(`proceduralBuilding.detail_${mode}`) }}
              </button>
            </div>
          </div>
        </div>

        <div class="panel-section">
          <h3>{{ $t("proceduralBuilding.sectionSeed") }}</h3>
          <div class="seed-row">
            <input
              v-model.number="seed"
              class="seed-input"
              type="number"
              min="0"
              max="4294967295"
              :aria-label="$t('proceduralBuilding.parameterSeed')"
            />
            <FTooltip :text="$t('proceduralBuilding.reseedHint')" side="top">
              <template #trigger>
                <button
                  type="button"
                  class="mini-btn"
                  :aria-label="$t('proceduralBuilding.reseed')"
                  @click="reseed"
                >
                  <FIcon name="RefreshCw" :size="14" aria-label="" />
                </button>
              </template>
            </FTooltip>
          </div>
        </div>

        <div class="panel-section">
          <h3>{{ $t("proceduralBuilding.sectionTextures") }}</h3>
          <div
            v-for="slot in MATERIAL_SLOTS"
            :key="slot.id"
            class="slot-row"
          >
            <img
              v-if="slotThumbs.get(slot.id)"
              class="slot-chip slot-chip-texture"
              :src="slotThumbs.get(slot.id)"
              alt=""
            />
            <span
              v-else
              class="slot-chip"
              :style="{ background: slot.defaultColor }"
            />
            <span class="slot-name">{{ $t(`proceduralBuilding.slot_${slot.id}`) }}</span>
            <span class="slot-binding" :title="slotLabels.get(slot.id)">
              {{ slotLabels.get(slot.id) ?? $t("proceduralBuilding.slotUnbound") }}
            </span>
            <button
              type="button"
              class="mini-btn"
              :aria-label="$t('proceduralBuilding.bindTexture')"
              @click="openPicker(slot.id)"
            >
              <FIcon name="Image" :size="13" aria-label="" />
            </button>
            <button
              v-if="bindings[slot.id]"
              type="button"
              class="mini-btn"
              :aria-label="$t('proceduralBuilding.clearTexture')"
              @click="unbindSlot(slot.id)"
            >
              <FIcon name="Eraser" :size="13" aria-label="" />
            </button>
          </div>
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
            <span class="slot-name">{{ $t("proceduralBuilding.lotGround") }}</span>
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

    <TexturePickerSheet v-model:open="pickerOpen" @pick="onPick" />
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
.procedural-page :deep(.f-sheet-embedded) { display: flex; flex: 1; min-height: 0; width: 100%; }
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
.panel-toggle:hover { background: var(--surface-hover); color: var(--foreground); }
.panel-toggle.active { border-color: var(--brand); color: var(--brand); }

.editor-reopen {
  align-items: center;
  display: flex;
  inset: 0;
  justify-content: center;
  pointer-events: none;
  position: absolute;
}
.editor-reopen :deep(.f-button) { pointer-events: auto; }

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
.panel-title { display: flex; flex-direction: column; gap: 4px; }
.panel-summary { font-size: 12px; opacity: 0.65; }
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
.param { align-items: center; display: grid; gap: 8px; grid-template-columns: 1fr 72px; }
.param span { font-size: 12px; }
.param input { grid-column: 1; width: 100%; }
.param em { font-size: 12px; font-style: normal; opacity: 0.7; text-align: right; }
.segment { display: flex; flex-direction: column; gap: 4px; }
.segment > span { font-size: 12px; }
.segment > div { display: flex; flex-wrap: wrap; gap: 4px; }
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
.seed-row { display: flex; gap: 6px; }
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
.mini-btn:hover { border-color: var(--primary); color: var(--primary); }
.slot-row { align-items: center; display: grid; gap: 6px; grid-template-columns: 16px auto 1fr auto auto; }
.slot-chip {
  border: 1px solid rgba(255, 255, 255, 0.25);
  border-radius: 3px;
  display: inline-block;
  height: 16px;
  object-fit: cover;
  width: 16px;
}
.slot-chip-empty { background: var(--surface-hover, rgba(255, 255, 255, 0.08)); }
.slot-row-spacer { flex: 1; }
.slot-name { font-size: 12px; }
.slot-binding {
  font-size: 11px;
  overflow: hidden;
  opacity: 0.65;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.panel-actions { margin-top: auto; }
.panel-hint { font-size: 11px; line-height: 1.5; opacity: 0.6; }
</style>
