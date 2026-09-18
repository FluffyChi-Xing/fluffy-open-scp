<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FDropdown from "@/components/ui/FDropdown.vue";
import FColorPicker from "@/components/ui/FColorPicker.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import RasterCanvas from "@/components/raster/RasterCanvas.vue";
import {
  Resizable,
  ResizableHandle,
  ResizablePanel,
} from "@/components/ui/resizable";
import { isTauri, tauriApi } from "@/api";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { useToast } from "@/composables/useToast";
import { RasterDocument } from "@/lib/raster-editor/document";
import { RasterHistory } from "@/lib/raster-editor/history";
import {
  base64ToRgba,
  rgbaToBase64,
  simulateDecalDataUrl,
  simulateLotDataUrl,
  layerOfPixel,
  LOT_LAYERS,
} from "@/lib/raster-editor/encoders";
import type { Rgba } from "@/lib/raster-editor/tools";
import type { RasterRgbaResponse, Tgi } from "@/api/tauri";

/**
 * Raster 绘制面板（开发工作台）：
 * 左侧 = 绘制工作台（工具 + 画布），右侧 = 来源面板（外部图片导入 /
 * 包内 Raster 选择），Resizable 控制两区宽度比。
 * 编辑一律基于「副本」：保存时写出 overlay package，不触碰源包。
 */
const { t } = useI18n();
const toast = useToast();
const gamePackages = useGamePackagesStore();

const RASTER_TYPE_ID = 0x2f4e681c;
type Tool =
  | "brush"
  | "eraser"
  | "line"
  | "rect"
  | "fill"
  | "picker"
  | "parking"
  | "curve";
const canvasRef = ref<InstanceType<typeof RasterCanvas> | null>(null);
const doc = shallowRef<RasterDocument | null>(null);
const history = shallowRef<RasterHistory | null>(null);
const docName = ref("");
const sourceTgi = ref<Tgi | null>(null);
const hasChanges = ref(false);
const canUndo = ref(false);
const canRedo = ref(false);
const saving = ref(false);
const tool = ref<Tool>("brush");
const brushSize = ref(4);
const colorHex = ref("#4f46e5");
/** 量化视图（默认）：所见即游戏使用的阈值化四色清晰结构。 */
const quantized = ref(true);
/** Lot 比例尺：1 px = 0.75 m（开启后显示米标尺与米坐标）。 */
const lotScale = ref(false);
/** 油漆桶容差：外部图片的抗锯齿渐变需要非零容差才能整片填充。 */
const fillTolerance = ref(32);
/** 停车位笔刷参数：默认取自游戏原生标线 0x7BE85E77 实测（0.75 m/px）。 */
const parkingLength = ref(13);
const parkingSpacing = ref(6);
/**
 * 四色通道模式（默认）：lot/decal mask 的通道即 LotColor1-4 材质层，
 * 只允许整层落笔；关闭后为自由 RGB（普通 overlay 贴图用）。
 */
const paletteMode = ref(true);
const layerIndex = ref(0);

const brushColor = computed<Rgba>(() => {
  if (paletteMode.value) {
    return [...LOT_LAYERS[layerIndex.value].paint] as Rgba;
  }
  const hex = colorHex.value.replace("#", "");
  return [
    parseInt(hex.slice(0, 2), 16),
    parseInt(hex.slice(2, 4), 16),
    parseInt(hex.slice(4, 6), 16),
    255,
  ];
});

const layerLabels = computed(() =>
  LOT_LAYERS.map((layer, index) => ({
    index,
    display: layer.display,
    label: t(`studio.raster.layer${layer.channel}`),
  })),
);

const tools = computed(
  () =>
    [
      { key: "brush", icon: "Brush", label: t("studio.raster.toolBrush") },
      { key: "eraser", icon: "Eraser", label: t("studio.raster.toolEraser") },
      { key: "line", icon: "Slash", label: t("studio.raster.toolLine") },
      { key: "curve", icon: "Spline", label: t("studio.raster.toolCurve") },
      { key: "rect", icon: "Square", label: t("studio.raster.toolRect") },
      {
        key: "parking",
        icon: "SquareParking",
        label: t("studio.raster.toolParking"),
      },
      { key: "fill", icon: "PaintBucket", label: t("studio.raster.toolFill") },
      { key: "picker", icon: "Pipette", label: t("studio.raster.toolPicker") },
    ] as const,
);

function syncHistoryState() {
  canUndo.value = history.value?.canUndo ?? false;
  canRedo.value = history.value?.canRedo ?? false;
}

const previewVersion = ref(0);
const previewLotUrl = ref("");
const previewDecalUrl = ref("");

function onChange() {
  hasChanges.value = true;
  previewVersion.value += 1;
  syncHistoryState();
}

function undo() {
  canvasRef.value?.undo();
}

function redo() {
  canvasRef.value?.redo();
}

function setEditor(document: RasterDocument, name: string, tgi: Tgi | null) {
  doc.value = document;
  history.value = new RasterHistory(document);
  docName.value = name;
  sourceTgi.value = tgi;
  hasChanges.value = false;
  syncHistoryState();
}

// ── 来源面板：Tab 与外部导入 ──

const sourceTab = ref<"external" | "package" | "preview">("external");
const external = ref<{
  name: string;
  width: number;
  height: number;
  rgbaBase64: string;
} | null>(null);
const externalLoading = ref(false);
const externalError = ref("");

async function chooseExternalImage() {
  const path = await tauriApi.workspace.pickImageFile(
    t("studio.raster.pickTitle"),
  );
  if (!path) return;
  externalLoading.value = true;
  externalError.value = "";
  try {
    const image = await tauriApi.raster.readImageRgba(path);
    external.value = {
      name: path.split(/[\\/]/).pop() ?? path,
      width: image.width,
      height: image.height,
      rgbaBase64: image.rgbaBase64,
    };
  } catch (cause) {
    external.value = null;
    externalError.value = messageOf(cause);
  } finally {
    externalLoading.value = false;
  }
}

function createFromExternal() {
  const image = external.value;
  if (!image) return;
  setEditor(
    new RasterDocument(
      image.width,
      image.height,
      base64ToRgba(image.rgbaBase64),
    ),
    `${t("studio.raster.copyOf", { name: image.name })}`,
    null,
  );
  toast.success(t("studio.raster.copyCreated"));
}

// ── 来源面板：包内 Raster ──

interface PackageRaster {
  tgi: Tgi;
  name: string;
  info: RasterRgbaResponse | null;
  previewUrl: string | null;
}

const selectedPackageId = ref<number | null>(null);
const packageMenuOpen = ref(false);
const packageRasters = shallowRef<PackageRaster[]>([]);
const loadingRasters = ref(false);
const selectedRaster = shallowRef<PackageRaster | null>(null);
const selectedInfo = shallowRef<RasterRgbaResponse | null>(null);
const selectedPreview = ref("");

const openedPackages = computed(() =>
  gamePackages.opened.map((opened) => opened.package),
);

function packageName(packageId: number): string {
  const opened = gamePackages.opened.find(
    (entry) => entry.package.packageId === packageId,
  );
  return opened?.package.path.split(/[\\/]/).pop() ?? String(packageId);
}

async function selectPackage(packageId: number | null) {
  selectedPackageId.value = packageId;
  selectedRaster.value = null;
  selectedInfo.value = null;
  selectedPreview.value = "";
  packageRasters.value = [];
  if (packageId === null) return;
  loadingRasters.value = true;
  try {
    const page = await tauriApi.packages.listResources(
      packageId,
      0,
      500,
      undefined,
      RASTER_TYPE_ID,
    );
    packageRasters.value = page.items.map((item) => ({
      tgi: item.tgi,
      name: hexLabel(item.tgi.instance),
      info: null,
      previewUrl: null,
    }));
  } finally {
    loadingRasters.value = false;
  }
}

async function selectRaster(item: PackageRaster) {
  selectedRaster.value = item;
  selectedInfo.value = null;
  selectedPreview.value = "";
  if (selectedPackageId.value === null) return;
  try {
    const [rgba, preview] = await Promise.all([
      tauriApi.raster.readRgba(selectedPackageId.value, item.tgi),
      tauriApi.packages.readRasterPreview(
        selectedPackageId.value,
        item.tgi,
        "composite",
      ),
    ]);
    selectedInfo.value = rgba;
    item.info = rgba;
    item.previewUrl = preview.pngBase64
      ? `data:image/png;base64,${preview.pngBase64}`
      : "";
    selectedPreview.value = item.previewUrl;
  } catch {
    selectedInfo.value = null;
  }
}

async function createFromPackage() {
  const item = selectedRaster.value;
  const info = selectedInfo.value;
  const packageId = selectedPackageId.value;
  if (!item || !info || !info.decodable || !info.rgbaBase64) {
    return;
  }
  setEditor(
    new RasterDocument(info.width, info.height, base64ToRgba(info.rgbaBase64)),
    `${t("studio.raster.copyOf", { name: item.name })}`,
    packageId === null ? null : { ...item.tgi },
  );
  toast.success(t("studio.raster.copyCreated"));
}

// ── 保存 ──

async function saveCopy() {
  const document = doc.value;
  if (!document || saving.value) return;
  saving.value = true;
  try {
    const fallbackInstance = fnv1a(`${docName.value}:${Date.now()}`);
    const tgi: Tgi = sourceTgi.value ?? {
      typeId: RASTER_TYPE_ID,
      group: 0,
      instance: fallbackInstance,
    };
    const path = await tauriApi.packages.saveFile(
      `${docName.value.replace(/\s+/g, "-")}.package`,
      "package",
    );
    if (!path) return;
    const result = await tauriApi.raster.saveOverlay({
      width: document.width,
      height: document.height,
      rgbaBase64: rgbaToBase64(document.pixels),
      tgi,
      outputPath: path,
      generateMips: true,
    });
    toast.success(
      t("studio.raster.saveSuccess", {
        path: result.outputPath,
        bytes: result.rasterBytes,
      }),
    );
    hasChanges.value = false;
  } finally {
    saving.value = false;
  }
}

function fnv1a(text: string): number {
  let hash = 0x811c9dc5;
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash >>> 0;
}

function hexLabel(value: number): string {
  return `0x${(value >>> 0).toString(16).padStart(8, "0").toUpperCase()}`;
}

function hexKey(tgi: Tgi): string {
  return `${tgi.typeId}:${tgi.group}:${tgi.instance}`;
}

const groundTileUrls = import.meta.glob<string>(
  "../../../../assets/ground/*.png",
  { eager: true, import: "default", query: "?url" },
) as unknown as Record<string, string>;

function grassTileUrl(): string | null {
  for (const [path, url] of Object.entries(groundTileUrls)) {
    if (path.endsWith("/8.png")) return url;
  }
  return null;
}

const isGrassReady = ref(false);
const grassImage = new Image();
grassImage.onload = () => (isGrassReady.value = true);
grassImage.src = grassTileUrl() ?? "";

watch([previewVersion, sourceTab, isGrassReady], renderPreviews);

function renderPreviews() {
  if (sourceTab.value !== "preview" || !doc.value) return;
  previewLotUrl.value = simulateLotDataUrl(
    doc.value,
    isGrassReady.value ? grassImage.src : null,
  );
  previewDecalUrl.value = simulateDecalDataUrl(doc.value);
}

function messageOf(cause: unknown): string {
  return cause && typeof cause === "object" && "message" in cause
    ? String(cause.message)
    : String(cause);
}

/** 取色：四色通道模式下吸附到层下标，自由模式下写 hex。 */
function onPickColor(rgba: Rgba) {
  if (paletteMode.value) {
    const layer = layerOfPixel(rgba);
    if (layer !== null) layerIndex.value = layer;
    return;
  }
  colorHex.value = `#${rgba[0]
    .toString(16)
    .padStart(2, "0")}${rgba[1].toString(16).padStart(2, "0")}${rgba[2]
    .toString(16)
    .padStart(2, "0")}`;
}

function metersOf(px: number): string {
  return `${(px * 0.75).toFixed(2)} m`;
}
</script>

<template>
  <section class="raster-page">
    <header class="page-heading">
      <div>
        <p class="eyebrow">{{ $t("studio.eyebrow") }}</p>
        <FTypography :header="1" spacing="none">{{
          $t("studio.raster.title")
        }}</FTypography>
        <FTypography paragraphy type="secondary">{{
          $t("studio.raster.description")
        }}</FTypography>
      </div>
      <button
        class="save-button"
        type="button"
        :disabled="!doc || !hasChanges || saving"
        @click="saveCopy"
      >
        <FIcon :name="saving ? 'Loader2' : 'Save'" :size="14" aria-label="" />
        {{ $t("studio.raster.save") }}
      </button>
    </header>

    <p v-if="!isTauri()" class="web-hint">{{ $t("studio.raster.webHint") }}</p>

    <Resizable
      v-else
      direction="horizontal"
      auto-save-id="openscp:raster-workbench:v1"
      class="workbench-shell"
    >
      <!-- 左：绘制工作台 -->
      <ResizablePanel id="raster-workbench" :default-size="62" :min-size="34">
        <div class="workbench" :class="{ empty: !doc }">
          <template v-if="doc && history">
            <div class="tool-strip">
              <div
                class="tool-group"
                role="toolbar"
                :aria-label="$t('studio.raster.toolsLabel')"
              >
                <button
                  v-for="entry in tools"
                  :key="entry.key"
                  type="button"
                  class="tool-button"
                  :class="{ active: tool === entry.key }"
                  :title="entry.label"
                  :aria-label="entry.label"
                  @click="tool = entry.key"
                >
                  <FIcon :name="entry.icon" :size="14" aria-label="" />
                </button>
              </div>
              <label class="size-group">
                <span>{{ $t("studio.raster.size") }}</span>
                <input
                  v-model.number="brushSize"
                  type="number"
                  min="1"
                  max="64"
                  class="size-input"
                />
              </label>
              <label v-if="tool === 'fill'" class="size-group">
                <span>{{ $t("studio.raster.tolerance") }}</span>
                <input
                  v-model.number="fillTolerance"
                  type="number"
                  min="0"
                  max="255"
                  class="size-input"
                  :title="$t('studio.raster.toleranceHint')"
                />
              </label>
              <template v-if="tool === 'parking'">
                <label class="size-group">
                  <span>{{ $t("studio.raster.parkingLength") }}</span>
                  <input
                    v-model.number="parkingLength"
                    type="number"
                    min="1"
                    max="512"
                    class="size-input"
                    :title="metersOf(parkingLength)"
                  />
                </label>
                <label class="size-group">
                  <span>{{ $t("studio.raster.parkingSpacing") }}</span>
                  <input
                    v-model.number="parkingSpacing"
                    type="number"
                    min="1"
                    max="512"
                    class="size-input"
                    :title="metersOf(parkingSpacing)"
                  />
                </label>
              </template>
              <div class="tool-group color-group">
                <template v-if="paletteMode">
                  <button
                    v-for="entry in layerLabels"
                    :key="entry.index"
                    type="button"
                    class="layer-swatch"
                    :class="{ active: layerIndex === entry.index }"
                    :style="{ background: entry.display }"
                    :title="entry.label"
                    :aria-label="entry.label"
                    @click="layerIndex = entry.index"
                  ></button>
                </template>
                <FColorPicker
                  v-else
                  v-model="colorHex"
                  :size="16"
                  :title="$t('studio.raster.color')"
                />
              </div>
              <button
                class="view-toggle"
                type="button"
                :class="{ active: paletteMode }"
                :title="$t('studio.raster.paletteHint')"
                @click="paletteMode = !paletteMode"
              >
                <FIcon name="Layers" :size="13" aria-label="" />
                {{
                  paletteMode
                    ? $t("studio.raster.paletteLot")
                    : $t("studio.raster.paletteFree")
                }}
              </button>
              <button
                class="view-toggle"
                type="button"
                :class="{ active: quantized }"
                :title="
                  quantized
                    ? $t('studio.raster.viewQuantizedHint')
                    : $t('studio.raster.viewRawHint')
                "
                @click="quantized = !quantized"
              >
                <FIcon
                  :name="quantized ? 'Eye' : 'EyeOff'"
                  :size="13"
                  aria-label=""
                />
                {{
                  quantized
                    ? $t("studio.raster.viewQuantized")
                    : $t("studio.raster.viewRaw")
                }}
              </button>
              <button
                class="view-toggle"
                type="button"
                :class="{ active: lotScale }"
                :title="$t('studio.raster.lotScaleHint')"
                @click="lotScale = !lotScale"
              >
                <FIcon name="Ruler" :size="13" aria-label="" />
                {{ $t("studio.raster.lotScale") }}
              </button>
              <span class="flex-spacer"></span>
              <button
                class="tool-button"
                type="button"
                :disabled="!canUndo"
                :title="$t('studio.raster.undo')"
                :aria-label="$t('studio.raster.undo')"
                @click="undo"
              >
                <FIcon name="Undo2" :size="14" aria-label="" />
              </button>
              <button
                class="tool-button"
                type="button"
                :disabled="!canRedo"
                :title="$t('studio.raster.redo')"
                :aria-label="$t('studio.raster.redo')"
                @click="redo"
              >
                <FIcon name="Redo2" :size="14" aria-label="" />
              </button>
            </div>
            <RasterCanvas
              ref="canvasRef"
              :doc="doc"
              :history="history"
              :tool="tool"
              :color="brushColor"
              :brush-size="brushSize"
              :fill-tolerance="fillTolerance"
              :parking-length="parkingLength"
              :parking-spacing="parkingSpacing"
              :quantized="quantized"
              :meters-per-pixel="lotScale ? 0.75 : null"
              @change="onChange"
              @pick-color="onPickColor"
            />
            <p v-if="docName" class="doc-name">
              <FIcon name="Copy" :size="12" aria-label="" />
              {{ docName }}
            </p>
            <p
              v-if="tool === 'curve' || tool === 'parking'"
              class="doc-name tool-usage"
            >
              <FIcon
                :name="tool === 'curve' ? 'Spline' : 'SquareParking'"
                :size="12"
                aria-label=""
              />
              {{
                tool === "curve"
                  ? $t("studio.raster.curveHint")
                  : $t("studio.raster.parkingHint")
              }}
            </p>
          </template>
          <div v-else class="workbench-empty">
            <FIcon name="Image" :size="28" aria-label="" />
            <p>{{ $t("studio.raster.empty") }}</p>
          </div>
        </div>
      </ResizablePanel>

      <ResizableHandle
        orientation="horizontal"
        :label="$t('studio.raster.resizeHint')"
      />

      <!-- 右：来源面板 -->
      <ResizablePanel id="raster-source" :default-size="38" :min-size="22">
        <div class="source-panel">
          <div
            class="source-tabs"
            role="tablist"
            :aria-label="$t('studio.raster.sourceLabel')"
          >
            <button
              class="source-tab"
              role="tab"
              :aria-selected="sourceTab === 'external'"
              :class="{ active: sourceTab === 'external' }"
              @click="sourceTab = 'external'"
            >
              {{ $t("studio.raster.tabExternal") }}
            </button>
            <button
              class="source-tab"
              role="tab"
              :aria-selected="sourceTab === 'package'"
              :class="{ active: sourceTab === 'package' }"
              @click="sourceTab = 'package'"
            >
              {{ $t("studio.raster.tabPackage") }}
            </button>
            <button
              class="source-tab"
              role="tab"
              :aria-selected="sourceTab === 'preview'"
              :class="{ active: sourceTab === 'preview' }"
              @click="sourceTab = 'preview'"
            >
              {{ $t("studio.raster.tabPreview") }}
            </button>
          </div>

          <!-- 外部导入 -->
          <div v-if="sourceTab === 'external'" class="source-body">
            <p class="source-hint">{{ $t("studio.raster.externalHint") }}</p>
            <button
              class="primary-button"
              type="button"
              @click="chooseExternalImage"
            >
              <FIcon
                :name="externalLoading ? 'Loader2' : 'FolderOpen'"
                :size="14"
                aria-label=""
              />
              {{ $t("studio.raster.pickImage") }}
            </button>
            <p v-if="externalError" class="error-text" role="alert">
              {{ externalError }}
            </p>
            <template v-if="external">
              <div class="source-preview checkerboard">
                <img
                  :src="`data:image/png;base64,${external.rgbaBase64}`"
                  :style="{ width: `${Math.min(220, external.width)}px` }"
                  alt=""
                />
              </div>
              <p class="source-meta">
                {{ external.name }} · {{ external.width }}×{{ external.height }}
              </p>
              <button
                class="primary-button wide"
                type="button"
                @click="createFromExternal"
              >
                <FIcon name="Copy" :size="14" aria-label="" />
                {{ $t("studio.raster.createCopy") }}
              </button>
            </template>
          </div>

          <!-- 渲染预览 -->
          <div v-else-if="sourceTab === 'preview'" class="source-body">
            <template v-if="doc">
              <section class="preview-card">
                <h3>{{ $t("studio.raster.previewLotTitle") }}</h3>
                <p class="preview-hint">
                  {{ $t("studio.raster.previewLotHint") }}
                </p>
                <div class="source-preview checkerboard">
                  <img v-if="previewLotUrl" :src="previewLotUrl" alt="" />
                </div>
              </section>
              <section class="preview-card">
                <h3>{{ $t("studio.raster.previewDecalTitle") }}</h3>
                <p class="preview-hint">
                  {{ $t("studio.raster.previewDecalHint") }}
                </p>
                <div class="source-preview checkerboard">
                  <img v-if="previewDecalUrl" :src="previewDecalUrl" alt="" />
                </div>
              </section>
              <p class="source-hint">
                {{ $t("studio.raster.previewDisclaimer") }}
              </p>
            </template>
            <p v-else class="source-hint">{{ $t("studio.raster.empty") }}</p>
          </div>

          <!-- 包内 Raster -->
          <div v-else class="source-body">
            <p class="source-hint">{{ $t("studio.raster.packageHint") }}</p>
            <FDropdown v-model:open="packageMenuOpen" :width="260">
              <template #trigger>
                <button type="button" class="package-trigger">
                  <span>{{
                    selectedPackageId === null
                      ? $t("studio.raster.pickPackage")
                      : packageName(selectedPackageId)
                  }}</span>
                  <FIcon name="ChevronDown" :size="12" aria-label="" />
                </button>
              </template>
              <button
                v-for="pkg in openedPackages"
                :key="pkg.packageId"
                type="button"
                @click="
                  selectPackage(pkg.packageId);
                  packageMenuOpen = false;
                "
              >
                <FIcon
                  :name="
                    selectedPackageId === pkg.packageId ? 'Check' : 'Package'
                  "
                  :size="14"
                  aria-label=""
                />
                {{ packageName(pkg.packageId) }}
              </button>
              <div v-if="!openedPackages.length" class="menu-empty">
                {{ $t("studio.raster.needPackage") }}
              </div>
            </FDropdown>

            <div v-if="loadingRasters" class="source-loading">
              {{ $t("common.loading") }}
            </div>
            <ul v-else-if="packageRasters.length" class="raster-list">
              <li v-for="item in packageRasters" :key="hexKey(item.tgi)">
                <button
                  class="raster-item"
                  type="button"
                  :class="{ selected: selectedRaster === item }"
                  @click="selectRaster(item)"
                >
                  <FIcon name="Image" :size="13" aria-label="" />
                  {{ item.name }}
                </button>
              </li>
            </ul>
            <p v-else class="source-hint">
              {{ $t("studio.raster.noRasters") }}
            </p>

            <template v-if="selectedRaster">
              <div class="source-preview checkerboard">
                <img v-if="selectedPreview" :src="selectedPreview" alt="" />
                <p v-else class="not-editable">
                  {{ $t("studio.raster.notEditable") }}
                </p>
              </div>
              <p v-if="selectedInfo" class="source-meta">
                {{ selectedInfo.width }}×{{ selectedInfo.height }} · pixFmt
                {{ selectedInfo.pixelFormat }} · mip {{ selectedInfo.mipCount }}
              </p>
              <button
                class="primary-button wide"
                type="button"
                :disabled="!selectedInfo?.decodable"
                @click="createFromPackage"
              >
                <FIcon name="Copy" :size="14" aria-label="" />
                {{ $t("studio.raster.createCopy") }}
              </button>
            </template>
          </div>
        </div>
      </ResizablePanel>
    </Resizable>
  </section>
</template>

<style scoped>
.raster-page {
  display: grid;
  gap: 16px;
}
.page-heading {
  align-items: center;
  display: flex;
  justify-content: space-between;
}
.eyebrow {
  color: var(--primary);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.08em;
  margin: 0 0 10px;
}
.save-button {
  align-items: center;
  background: var(--primary);
  border: 1px solid var(--primary);
  border-radius: var(--radius-sm);
  color: var(--primary-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 6px;
  min-height: 32px;
  padding: 0 14px;
}
.save-button:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 85%, #fff);
}
.save-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.web-hint {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  color: var(--muted-foreground);
  font-size: 12.5px;
  padding: 26px 12px;
  text-align: center;
}
.workbench-shell {
  height: min(720px, calc(100vh - 240px));
  min-height: 480px;
}
.workbench {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  min-height: 0;
}
.workbench.empty {
  justify-content: center;
}
.tool-strip {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  padding: 8px 10px;
}
.tool-group {
  display: inline-flex;
  gap: 2px;
}
.tool-button {
  align-items: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 28px;
  min-width: 28px;
}
.tool-button:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--foreground);
}
.tool-button.active {
  background: var(--accent);
  border-color: var(--primary);
  color: var(--primary);
}
.tool-button:disabled {
  cursor: not-allowed;
  opacity: 0.4;
}
.size-group {
  align-items: center;
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 11.5px;
  gap: 6px;
}
.size-input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  min-height: 28px;
  padding: 0 8px;
  width: 56px;
}
.color-group :deep(.color-trigger) {
  justify-content: center;
  min-height: 28px;
  width: 36px;
}
.color-group :deep(.color-swatch) {
  width: 100%;
}
.layer-swatch {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  height: 20px;
  min-height: 20px;
  width: 20px;
}
.layer-swatch.active {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
.tool-usage {
  color: var(--subtle-foreground);
}
.flex-spacer {
  flex: 1;
}
.view-toggle {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11.5px;
  gap: 5px;
  min-height: 28px;
  padding: 0 10px;
}
.view-toggle.active {
  border-color: color-mix(in srgb, var(--primary) 55%, var(--border));
  color: var(--primary);
}
.preview-card {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: grid;
  gap: 6px;
  padding: 12px;
}
.preview-card h3 {
  font-size: 12.5px;
  margin: 0;
}
.preview-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 0;
}
.doc-name {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 11.5px;
  gap: 6px;
  margin: 0;
}
.workbench-empty {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex: 1;
  flex-direction: column;
  font-size: 12.5px;
  gap: 10px;
  justify-content: center;
  text-align: center;
}
.source-panel {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.source-tabs {
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 2px;
  padding: 8px 10px 0;
}
.source-tab {
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 12.5px;
  min-height: 30px;
  padding: 0 10px;
}
.source-tab.active {
  border-bottom-color: var(--primary);
  color: var(--foreground);
  font-weight: 700;
}
.source-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow: auto;
  padding: 12px;
}
.source-hint {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
}
.primary-button {
  align-items: center;
  background: var(--primary);
  border: 1px solid var(--primary);
  border-radius: var(--radius-sm);
  color: var(--primary-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 6px;
  justify-content: center;
  min-height: 32px;
  padding: 0 12px;
}
.primary-button.wide {
  width: 100%;
}
.primary-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.error-text {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  overflow-wrap: anywhere;
}
.source-preview {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  justify-content: center;
  max-height: 260px;
  min-height: 80px;
  overflow: hidden;
  padding: 8px;
}
.source-preview img {
  image-rendering: pixelated;
  max-width: 100%;
}
.source-meta {
  color: var(--muted-foreground);
  font-size: 11.5px;
  font-variant-numeric: tabular-nums;
  margin: 0;
}
.not-editable {
  color: var(--warning);
  font-size: 12px;
  margin: 0;
}
.package-trigger {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 8px;
  justify-content: space-between;
  min-height: 32px;
  padding: 0 10px;
  width: 100%;
}
.raster-list {
  display: grid;
  gap: 2px;
  list-style: none;
  margin: 0;
  max-height: 260px;
  overflow: auto;
  padding: 0;
}
.raster-item {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 8px;
  min-height: 30px;
  padding: 0 8px;
  text-align: start;
  width: 100%;
}
.raster-item:hover {
  background: var(--surface-hover);
}
.raster-item.selected {
  background: var(--accent);
  color: var(--primary);
}
.source-loading {
  color: var(--muted-foreground);
  font-size: 12px;
  padding: 16px 0;
  text-align: center;
}
</style>
