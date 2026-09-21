<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FDropdown from "@/components/ui/FDropdown.vue";
import FColorPicker from "@/components/ui/FColorPicker.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSheet from "@/components/ui/FSheet.vue";
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
  composeLotMaterialDataUrl,
  simulateDecalDataUrl,
  simulateLotDataUrl,
  layerOfPixel,
  makeCheckerboard,
  rgbaBase64ToPngDataUrl,
  scaleRgba,
  fitSizeWithin,
  LOT_LAYERS,
} from "@/lib/raster-editor/encoders";
import type { Rgba } from "@/lib/raster-editor/tools";
import type {
  LotMaterialResponse,
  PropertyDocumentSummary,
  RasterRgbaResponse,
  Tgi,
} from "@/api/tauri";

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

const sourceTab = ref<"external" | "package" | "preview" | "decal">(
  "external",
);
const external = ref<{
  name: string;
  width: number;
  height: number;
  rgbaBase64: string;
  /** PNG data URL 预览（后端下发的是裸 RGBA，不能直接当 img src）。 */
  previewUrl: string;
} | null>(null);
const externalLoading = ref(false);
const externalError = ref("");

/**
 * 游戏 lot/decal 的最大 footprint：1px = 0.75m，最大地块单边 ≈ 256px
 * （192m）。四色通道（lot/decal 语义）导入强制缩放到该范围内。
 */
const MAX_LOT_PX = 256;
const SIZE_PRESETS = [32, 64, 128, 192, 256] as const;
const targetWidth = ref(0);
const targetHeight = ref(0);
const lockAspect = ref(true);

function clampSize(value: number): number {
  if (!Number.isFinite(value)) return 1;
  return Math.min(4096, Math.max(1, Math.round(value)));
}

async function chooseExternalImage() {
  const path = await tauriApi.workspace.pickImageFile(
    t("studio.raster.pickTitle"),
  );
  if (!path) return;
  externalLoading.value = true;
  externalError.value = "";
  try {
    const image = await tauriApi.raster.readImageRgba(path);
    const fitted = fitSizeWithin(image.width, image.height, MAX_LOT_PX);
    external.value = {
      name: path.split(/[\\/]/).pop() ?? path,
      width: image.width,
      height: image.height,
      rgbaBase64: image.rgbaBase64,
      previewUrl: rgbaBase64ToPngDataUrl(
        image.rgbaBase64,
        image.width,
        image.height,
      ),
    };
    targetWidth.value = fitted.width;
    targetHeight.value = fitted.height;
  } catch (cause) {
    external.value = null;
    externalError.value = messageOf(cause);
  } finally {
    externalLoading.value = false;
  }
}

/** 锁比例时只在改动方向上调另一边，避免双向联动抖动。 */
function onTargetWidthInput() {
  targetWidth.value = clampSize(targetWidth.value);
  if (lockAspect.value && external.value) {
    targetHeight.value = clampSize(
      Math.round(
        (targetWidth.value * external.value.height) / external.value.width,
      ),
    );
  }
}

function onTargetHeightInput() {
  targetHeight.value = clampSize(targetHeight.value);
  if (lockAspect.value && external.value) {
    targetWidth.value = clampSize(
      Math.round(
        (targetHeight.value * external.value.width) / external.value.height,
      ),
    );
  }
}

function applySizePreset(max: number) {
  if (!external.value) return;
  const fitted = fitSizeWithin(
    external.value.width,
    external.value.height,
    max,
  );
  targetWidth.value = fitted.width;
  targetHeight.value = fitted.height;
}

function createFromExternal() {
  const image = external.value;
  if (!image) return;
  const width = clampSize(targetWidth.value);
  const height = clampSize(targetHeight.value);
  // 四色通道 = lot/decal 语义，超出游戏最大 footprint 没有意义，强制缩放。
  if (Math.max(width, height) > MAX_LOT_PX && paletteMode.value) {
    toast.error(t("studio.raster.importTooLarge", { max: MAX_LOT_PX }));
    return;
  }
  targetLot.value = null;
  setEditor(
    new RasterDocument(
      width,
      height,
      scaleRgba(
        base64ToRgba(image.rgbaBase64),
        image.width,
        image.height,
        width,
        height,
      ),
    ),
    `${t("studio.raster.copyOf", { name: image.name })} · ${width}×${height}`,
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
  targetLot.value = null;
  setEditor(
    new RasterDocument(info.width, info.height, base64ToRgba(info.rgbaBase64)),
    `${t("studio.raster.copyOf", { name: item.name })}`,
    packageId === null ? null : { ...item.tgi },
  );
  toast.success(t("studio.raster.copyCreated"));
}

// ── 覆盖目标 Lot（绘制 → 覆盖导出闭环） ──

/** 跨包扫描的文档：携带来源 packageId（读 LotMask / 伴随 property / 注册都用它）。 */
type PackageDocument = PropertyDocumentSummary & { packageId: number };

interface TargetLot {
  summary: PackageDocument;
  maskWidth: number;
  maskHeight: number;
  /** 载入时实际命中的解析 TGI（完整 raster TGI），覆盖导出沿用。 */
  maskTgi: Tgi;
  /** 覆盖目标的 LotColor1-4 授权（预览染色用）；null = 读取失败走缺省色。 */
  material: LotMaterialResponse | null;
}
const targetLot = shallowRef<TargetLot | null>(null);
const lotMenuOpen = ref(false);
const lotDocuments = shallowRef<PackageDocument[]>([]);
const docsLoading = ref(false);
/** 下拉一次渲染的条数上限（大包 lot 文档数千，超出部分提示用搜索缩小范围）。 */
const LOT_MENU_LIMIT = 300;

const targetAtlas = shallowRef<PackageDocument | null>(null);
const atlasMenuOpen = ref(false);
const decalAtlases = shallowRef<PackageDocument[]>([]);

let scannedSignature = "";

function openedSignature(): string {
  return gamePackages.opened
    .map((entry) => entry.package.packageId)
    .sort((a, b) => a - b)
    .join(",");
}

function sameDocument(document: PackageDocument, target: PackageDocument) {
  return (
    document.packageId === target.packageId &&
    document.tgi.instance === target.tgi.instance &&
    document.tgi.group === target.tgi.group
  );
}

/** 扫描所有已打开 package 的 lot / decal atlas 文档（不再局限于单个包）。 */
async function scanDocuments(force = false) {
  const signature = openedSignature();
  const packages = gamePackages.opened.map((entry) => entry.package);
  if (!packages.length) {
    scannedSignature = "";
    lotDocuments.value = [];
    decalAtlases.value = [];
    targetLot.value = null;
    targetAtlas.value = null;
    return;
  }
  if (!force && signature === scannedSignature) return;
  scannedSignature = signature;
  docsLoading.value = true;
  try {
    const scanKind = async (kind: "lot" | "decal") =>
      (
        await Promise.all(
          packages.map((pkg) =>
            tauriApi.raster
              .listPropertyDocuments(pkg.packageId, kind)
              .catch((cause) => {
                // 扫描失败不能再静默吞掉：上次参数不匹配就因此不可见。
                console.error("[raster] 文档扫描失败", pkg.packageId, kind, cause);
                return [] as PropertyDocumentSummary[];
              })
              .then((documents) =>
                documents.map((document) => ({
                  ...document,
                  packageId: pkg.packageId,
                })),
              ),
          ),
        )
      ).flat();
    const [lots, atlases] = await Promise.all([
      scanKind("lot"),
      scanKind("decal"),
    ]);
    lotDocuments.value = lots;
    decalAtlases.value = atlases;
    // 目标若来自已关闭的 package，清空失效选择。
    const selectedLot = targetLot.value;
    if (selectedLot && !lots.some((d) => sameDocument(d, selectedLot.summary))) {
      targetLot.value = null;
    }
    const selectedAtlas = targetAtlas.value;
    if (selectedAtlas && !atlases.some((d) => sameDocument(d, selectedAtlas))) {
      targetAtlas.value = null;
    }
  } finally {
    docsLoading.value = false;
  }
}

watch(
  sourceTab,
  (tab) => {
    if (tab === "package" || tab === "decal") void scanDocuments();
  },
  { immediate: true },
);

function lotLabel(summary: PackageDocument): string {
  const size = summary.lotSize
    ? ` · ${summary.lotSize[0]}×${summary.lotSize[1]}m`
    : "";
  return `${packageName(summary.packageId)} · ${hexLabel(summary.tgi.instance)}${size}`;
}

/**
 * 选择覆盖目标：载入该 Lot 的 LotMask 作为编辑副本（保持原宽高 / pixFmt21），
 * 保存时同包写回源 lot property，导出包即可入游戏覆盖地面。
 * SCP 的 LotMask key 惯例为残缺 TGI（type/group=0），读取与导出必须用
 * 服务端解析出的完整 raster TGI（lotMaskResolved）；候选顺序 = 解析包 →
 * 来源包 → 其余已打开包（后两者是解析缺失时的兜底）。
 */
async function chooseTargetLot(summary: PackageDocument) {
  lotMenuOpen.value = false;
  const mask = summary.lotMaskResolved ?? summary.lotMask;
  if (!mask) return;
  const candidates: number[] = [];
  if (summary.lotMaskPackageId !== null) candidates.push(summary.lotMaskPackageId);
  for (const packageId of [
    summary.packageId,
    ...gamePackages.opened.map((entry) => entry.package.packageId),
  ]) {
    if (!candidates.includes(packageId)) candidates.push(packageId);
  }
  let loaded: { width: number; height: number; rgba: string } | null = null;
  for (const packageId of candidates) {
    try {
      const attempt = await tauriApi.raster.readRgba(packageId, mask);
      if (attempt.decodable && attempt.rgbaBase64) {
        loaded = {
          width: attempt.width,
          height: attempt.height,
          rgba: attempt.rgbaBase64,
        };
        break;
      }
    } catch (cause) {
      // 该包没有此 mask（或不可读），继续在其余包里找；全部失败时在
      // console 留下诊断信息。
      console.warn("[raster] LotMask 读取失败", packageId, cause);
    }
  }
  if (!loaded) {
    toast.error(t("studio.raster.lotMaskMissing"));
    return;
  }
  // 替换材质授权：读源 lot 的 LotColor1-4（挂在父级时服务端已展平）。
  // 失败不阻断载入——渲染预览回退 SCP 缺省色。
  let material: LotMaterialResponse | null = null;
  try {
    material = await tauriApi.raster.readLotMaterial(summary.packageId, {
      ...summary.tgi,
    });
  } catch (cause) {
    console.warn("[raster] LotColor 读取失败", summary.packageId, cause);
  }
  targetLot.value = null;
  setEditor(
    new RasterDocument(
      loaded.width,
      loaded.height,
      base64ToRgba(loaded.rgba),
    ),
    `${t("studio.raster.copyOf", { name: lotLabel(summary) })}`,
    { ...mask },
  );
  targetLot.value = {
    summary,
    maskWidth: loaded.width,
    maskHeight: loaded.height,
    maskTgi: { ...mask },
    material,
  };
  toast.success(
    t("studio.raster.lotLoaded", {
      size: `${loaded.width}×${loaded.height}`,
    }),
  );
}

// ── Decal 注册（绘制 → 注册 → 导出闭环） ──

const registering = ref(false);
const decalEntryId = ref("");
const decalAspectRatio = ref(1);
/** Color1-4（sRGB hex，落盘前换算为线性半值存储口径）。 */
const decalColors = ref(["#ffffff", "#ffffff", "#ffffff", "#ffffff"]);
const decalReplace = ref(false);
const decalReplaceInstance = ref("");

// ── 新建 Lot（画布 → raster + 最小 Lot property 同包导出） ──

const PROPERTY_TYPE_ID = 0x00b1b104;
const newLotOpen = ref(false);
const creatingLot = ref(false);
/** LotColor1-4 默认取 SCP 回退色（LC1 黑 / LC2 红 / LC3 绿 / LC4 蓝），索引 0。 */
const newLotColors = ref([
  { hex: "#000000", tile: 0 },
  { hex: "#ff0000", tile: 0 },
  { hex: "#00ff00", tile: 0 },
  { hex: "#0000ff", tile: 0 },
]);

/** 表单 LC1-4 → [sRGB RGB + A=tile]（预览染料与材质合成的无目标回退源）。 */
const newLotColorRgba = computed<[number, number, number, number][]>(() =>
  newLotColors.value.map((color) => [...hexToSrgbBytes(color.hex), color.tile]),
);

function hexToSrgbBytes(hex: string): [number, number, number] {
  const value = hex.replace("#", "");
  return [
    Number.parseInt(value.slice(0, 2), 16) || 0,
    Number.parseInt(value.slice(2, 4), 16) || 0,
    Number.parseInt(value.slice(4, 6), 16) || 0,
  ];
}

async function createLot() {
  const document = doc.value;
  if (!document || creatingLot.value) return;
  creatingLot.value = true;
  try {
    const path = await tauriApi.packages.saveFile(
      `lot-${docName.value.replace(/\s+/g, "-")}.package`,
      "package",
    );
    if (!path) return;
    const seed = `${docName.value}:${Date.now()}`;
    const result = await tauriApi.raster.createLotOverlay({
      width: document.width,
      height: document.height,
      rgbaBase64: rgbaToBase64(document.pixels),
      generateMips: true,
      rasterTgi: {
        typeId: RASTER_TYPE_ID,
        group: 0,
        instance: fnv1a(`lot-raster:${seed}`),
      },
      lotTgi: {
        typeId: PROPERTY_TYPE_ID,
        group: 0,
        instance: fnv1a(`lot-property:${seed}`),
      },
      colors: newLotColors.value.map(
        (color) => [...hexToSrgbBytes(color.hex), color.tile] as [number, number, number, number],
      ),
      outputPath: path,
    });
    toast.success(
      t("studio.raster.createLotSuccess", {
        size: `${result.lotSize[0]}×${result.lotSize[1]}m`,
        path: result.outputPath,
      }),
    );
  } catch (cause) {
    toast.error(messageOf(cause));
  } finally {
    creatingLot.value = false;
  }
}

function atlasLabel(summary: PackageDocument): string {
  return `${packageName(summary.packageId)} · ${hexLabel(summary.tgi.instance)} · ${t("decal.entryCount")} ${
    summary.entryCount ?? 0
  }`;
}

function syncDecalAspect() {
  const document = doc.value;
  if (document && document.height > 0) {
    decalAspectRatio.value = Number(
      (document.width / document.height).toFixed(4),
    );
  }
}

watch(doc, syncDecalAspect);

/** sRGB 8bit → 线性分量 → 存储口径（线性值的一半，与既有字典一致）。 */
function srgbToStorage(u8: number): number {
  const srgb = u8 / 255;
  const linear =
    srgb <= 0.04045 ? srgb / 12.92 : Math.pow((srgb + 0.055) / 1.055, 2.4);
  return linear / 2;
}

function hexToStorage(hex: string): [number, number, number, number] {
  const value = hex.replace("#", "");
  return [
    srgbToStorage(parseInt(value.slice(0, 2), 16) || 0),
    srgbToStorage(parseInt(value.slice(2, 4), 16) || 0),
    srgbToStorage(parseInt(value.slice(4, 6), 16) || 0),
    0,
  ];
}

function parseHexInstance(text: string): number | null {
  const value = Number.parseInt(text.trim().replace(/^0x/i, ""), 16);
  return Number.isFinite(value) && value > 0 ? value >>> 0 : null;
}

async function registerDecal() {
  const document = doc.value;
  const atlas = targetAtlas.value;
  if (!document || !atlas || registering.value) return;
  const entryInstance =
    parseHexInstance(decalEntryId.value) ?? fnv1a(`${docName.value}:${Date.now()}`);
  const replaceInstance = decalReplace.value
    ? parseHexInstance(decalReplaceInstance.value)
    : null;
  if (decalReplace.value && replaceInstance === null) {
    toast.error(t("studio.raster.decalReplaceIdRequired"));
    return;
  }
  registering.value = true;
  try {
    const path = await tauriApi.packages.saveFile(
      `decal-${entryInstance.toString(16)}.package`,
      "package",
    );
    if (!path) return;
    const result = await tauriApi.raster.registerDecalEntry({
      width: document.width,
      height: document.height,
      rgbaBase64: rgbaToBase64(document.pixels),
      generateMips: true,
      atlas: { packageId: atlas.packageId, tgi: { ...atlas.tgi } },
      rasterTgi: {
        typeId: RASTER_TYPE_ID,
        group: 0,
        instance: fnv1a(`decal-raster:${entryInstance}:${Date.now()}`),
      },
      entryId: { typeId: 0, group: 0, instance: entryInstance },
      aspectRatio: decalAspectRatio.value,
      colors: decalColors.value.map(hexToStorage),
      replaceInstance: replaceInstance ?? undefined,
      outputPath: path,
    });
    toast.success(
      t("studio.raster.decalSuccess", {
        index: result.entryIndex + 1,
        count: result.entryCount,
        path: result.outputPath,
      }),
    );
  } catch (cause) {
    toast.error(messageOf(cause));
  } finally {
    registering.value = false;
  }
}

// ── 保存 ──

async function saveCopy() {
  const document = doc.value;
  if (!document || saving.value) return;
  const lot = targetLot.value;
  // 覆盖导出必须保持 LotMask 原宽高（游戏按 tile 对齐采样）。
  if (lot && (document.width !== lot.maskWidth || document.height !== lot.maskHeight)) {
    toast.error(
      t("studio.raster.dimMismatch", {
        w: lot.maskWidth,
        h: lot.maskHeight,
      }),
    );
    return;
  }
  saving.value = true;
  try {
    const fallbackInstance = fnv1a(`${docName.value}:${Date.now()}`);
    // 覆盖目标必须用载入时解析出的完整 raster TGI：源 lot 的 LotMask key
    // 惯例 type/group=0，服务端会拒绝非 raster 类型，游戏也按 raster 类型
    // 定位覆盖资源。
    const tgi: Tgi = lot
      ? { ...lot.maskTgi }
      : (sourceTgi.value ?? {
          typeId: RASTER_TYPE_ID,
          group: 0,
          instance: fallbackInstance,
        });
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
      companionProperty: lot
        ? { packageId: lot.summary.packageId, tgi: { ...lot.summary.tgi } }
        : undefined,
    });
    toast.success(
      t("studio.raster.saveSuccess", {
        path: result.outputPath,
        bytes: result.rasterBytes,
      }),
    );
    hasChanges.value = false;
  } catch (cause) {
    toast.error(messageOf(cause));
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

watch([previewVersion, sourceTab, isGrassReady, doc], renderPreviews);

function renderPreviews() {
  if (sourceTab.value !== "preview" || !doc.value) return;
  previewLotUrl.value = simulateLotDataUrl(
    doc.value,
    isGrassReady.value ? grassImage.src : null,
    lotDyeColors.value,
  );
  previewDecalUrl.value = simulateDecalDataUrl(doc.value);
}

/**
 * 渲染预览染料：覆盖目标的 LotColor1-4（sRGB RGB）优先；未载入目标时用
 * 新建 Lot 表单声明的 LC1-4（缺省值 = SCP 黑/红/绿/蓝，未改动前观感不变）。
 */
const lotDyeColors = computed<
  readonly (readonly [number, number, number])[]
>(() => {
  const material = targetLot.value?.material;
  if (material) return material.colors.map(([r, g, b]) => [r, g, b] as const);
  return newLotColorRgba.value.map(([r, g, b]) => [r, g, b] as const);
});

// ── 渲染预览全屏 sheet ──

const previewSheet = ref<null | "lot" | "decal">(null);
type SheetZoom = "fit" | 1 | 2 | 4;
const sheetZoom = ref<SheetZoom>("fit");
/** Lot sheet 的材质合成开关：默认 = 染料平色；开启 = Lot Textures 图集铺贴。 */
const sheetMaterial = ref(false);
const sheetLotUrl = ref("");
const sheetOpen = computed({
  get: () => previewSheet.value !== null,
  set: (value: boolean) => {
    if (!value) previewSheet.value = null;
  },
});
const ZOOM_OPTIONS: { value: SheetZoom; labelKey: string }[] = [
  { value: "fit", labelKey: "studio.raster.zoomFit" },
  { value: 1, labelKey: "studio.raster.zoom1to1" },
  { value: 2, labelKey: "studio.raster.zoom2x" },
  { value: 4, labelKey: "studio.raster.zoom4x" },
];
const sheetCheckerboard = makeCheckerboard();

const sheetTitle = computed(() =>
  previewSheet.value === "decal"
    ? t("studio.raster.previewDecalTitle")
    : t("studio.raster.previewLotTitle"),
);
const sheetHint = computed(() =>
  previewSheet.value === "decal"
    ? t("studio.raster.previewDecalHint")
    : t("studio.raster.previewLotHint"),
);

function openPreviewSheet(kind: "lot" | "decal") {
  previewSheet.value = kind;
  sheetZoom.value = "fit";
  // sheet 从预览卡片打开，URL 理应新鲜；重渲一次兜底文档替换未触发 watch 的路径。
  renderPreviews();
  void syncSheetPreview();
}

/** Lot sheet 内容随开关/画布切换：默认走染料平色，开启材质走图集合成。 */
watch([sheetOpen, sheetMaterial, previewVersion], () => {
  void syncSheetPreview();
});

async function syncSheetPreview() {
  if (!sheetOpen.value || previewSheet.value !== "lot" || !doc.value) return;
  if (!sheetMaterial.value) {
    sheetLotUrl.value =
      previewLotUrl.value ||
      simulateLotDataUrl(
        doc.value,
        isGrassReady.value ? grassImage.src : null,
        lotDyeColors.value,
      );
    return;
  }
  const material = targetLot.value?.material;
  sheetLotUrl.value = await composeLotMaterialDataUrl({
    doc: doc.value,
    lotColors: material ? material.colors : newLotColorRgba.value,
    lotColorsAuthored: material
      ? [...material.colorsAuthored]
      : [true, true, true, true],
    surfaceUrl: material?.surfacePng
      ? `data:image/png;base64,${material.surfacePng}`
      : null,
    tilePeriod: material?.tilePeriod ?? null,
  });
}

const sheetImgStyle = computed(() => {
  if (!doc.value || sheetZoom.value === "fit") {
    return { maxWidth: "100%", maxHeight: "100%" };
  }
  return {
    width: `${doc.value.width * sheetZoom.value}px`,
    maxWidth: "none",
    maxHeight: "none",
  };
});

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
            <button
              class="source-tab"
              role="tab"
              :aria-selected="sourceTab === 'decal'"
              :class="{ active: sourceTab === 'decal' }"
              @click="
                sourceTab = 'decal';
                syncDecalAspect();
              "
            >
              {{ $t("studio.raster.tabDecal") }}
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
                  v-if="external.previewUrl"
                  :src="external.previewUrl"
                  :style="{ width: `${Math.min(220, external.width)}px` }"
                  alt=""
                />
              </div>
              <p class="source-meta">
                {{ external.name }} · {{ external.width }}×{{ external.height }}
              </p>
              <div class="form-field">
                <span>{{ $t("studio.raster.importSize") }}</span>
                <div class="size-row">
                  <input
                    v-model.number="targetWidth"
                    type="number"
                    min="1"
                    max="4096"
                    class="form-input"
                    @change="onTargetWidthInput"
                  />
                  <span class="size-x">×</span>
                  <input
                    v-model.number="targetHeight"
                    type="number"
                    min="1"
                    max="4096"
                    class="form-input"
                    @change="onTargetHeightInput"
                  />
                  <label class="form-check lock-aspect">
                    <input v-model="lockAspect" type="checkbox" />
                    <span>{{ $t("studio.raster.lockAspect") }}</span>
                  </label>
                </div>
                <div class="preset-row">
                  <button
                    v-for="preset in SIZE_PRESETS"
                    :key="preset"
                    type="button"
                    class="preset-button"
                    @click="applySizePreset(preset)"
                  >
                    ≤{{ preset }}px · {{ metersOf(preset) }}
                  </button>
                </div>
                <p
                  class="source-hint"
                  :class="{ warning: Math.max(targetWidth, targetHeight) > MAX_LOT_PX }"
                >
                  {{
                    $t("studio.raster.importSizeHint", {
                      max: MAX_LOT_PX,
                      maxM: MAX_LOT_PX * 0.75,
                    })
                  }}
                </p>
              </div>
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
                <div class="preview-card-head">
                  <h3>{{ $t("studio.raster.previewLotTitle") }}</h3>
                  <button
                    class="sheet-open"
                    type="button"
                    :title="$t('studio.raster.previewExpand')"
                    :aria-label="$t('studio.raster.previewExpand')"
                    @click="openPreviewSheet('lot')"
                  >
                    <FIcon name="Maximize" :size="13" aria-label="" />
                  </button>
                </div>
                <p class="preview-hint">
                  {{ $t("studio.raster.previewLotHint") }}
                </p>
                <div class="source-preview checkerboard">
                  <img v-if="previewLotUrl" :src="previewLotUrl" alt="" />
                </div>
              </section>
              <section class="preview-card">
                <div class="preview-card-head">
                  <h3>{{ $t("studio.raster.previewDecalTitle") }}</h3>
                  <button
                    class="sheet-open"
                    type="button"
                    :title="$t('studio.raster.previewExpand')"
                    :aria-label="$t('studio.raster.previewExpand')"
                    @click="openPreviewSheet('decal')"
                  >
                    <FIcon name="Maximize" :size="13" aria-label="" />
                  </button>
                </div>
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

            <!-- 全屏 sheet：大图浏览细节（棋盘透明底 + 缩放档位） -->
            <FSheet v-model:open="sheetOpen" width="100vw" :label="sheetTitle">
              <div class="preview-sheet">
                <header class="preview-sheet-head">
                  <div>
                    <FTypography :header="2" spacing="none">{{
                      sheetTitle
                    }}</FTypography>
                    <p class="preview-hint">{{ sheetHint }}</p>
                  </div>
                  <button
                    class="sheet-open"
                    type="button"
                    :title="$t('studio.raster.previewCollapse')"
                    :aria-label="$t('studio.raster.previewCollapse')"
                    @click="sheetOpen = false"
                  >
                    <FIcon name="X" :size="16" aria-label="" />
                  </button>
                </header>
                <div class="preview-sheet-toolbar">
                  <!-- 材质切换：与 property editor 精细渲染同款分段组件 -->
                  <div
                    v-if="previewSheet === 'lot'"
                    class="render-mode"
                    role="group"
                    :aria-label="$t('studio.raster.materialToggle')"
                  >
                    <button
                      type="button"
                      :class="{ active: !sheetMaterial }"
                      @click="sheetMaterial = false"
                    >
                      {{ $t("studio.raster.materialDefault") }}
                    </button>
                    <button
                      type="button"
                      :class="{ active: sheetMaterial }"
                      @click="sheetMaterial = true"
                    >
                      {{ $t("studio.raster.materialOn") }}
                    </button>
                  </div>
                  <button
                    v-for="option in ZOOM_OPTIONS"
                    :key="option.labelKey"
                    class="zoom-option"
                    type="button"
                    :class="{ active: sheetZoom === option.value }"
                    @click="sheetZoom = option.value"
                  >
                    {{ $t(option.labelKey) }}
                  </button>
                  <span v-if="doc" class="source-meta">
                    {{ doc.width }}×{{ doc.height }} · 1px = 0.75m
                  </span>
                </div>
                <div
                  class="preview-sheet-stage"
                  :style="{ backgroundImage: `url(${sheetCheckerboard})` }"
                >
                  <img
                    v-if="previewSheet === 'decal' ? previewDecalUrl : sheetLotUrl"
                    :src="previewSheet === 'decal' ? previewDecalUrl : sheetLotUrl"
                    :style="sheetImgStyle"
                    alt=""
                  />
                </div>
              </div>
            </FSheet>
          </div>

          <!-- Decal 注册 -->
          <div v-else-if="sourceTab === 'decal'" class="source-body">
            <p class="source-hint">{{ $t("studio.raster.decalHint") }}</p>

            <FDropdown v-model:open="atlasMenuOpen" :width="360">
              <template #trigger>
                <button type="button" class="package-trigger">
                  <span>
                    {{
                      targetAtlas
                        ? atlasLabel(targetAtlas)
                        : $t("studio.raster.pickAtlas")
                    }}
                  </span>
                  <FIcon name="ChevronDown" :size="12" aria-label="" />
                </button>
              </template>
              <div class="menu-scroll">
                <button
                  v-for="summary in decalAtlases"
                  :key="`${summary.packageId}:${hexKey(summary.tgi)}`"
                  type="button"
                  @click="
                    targetAtlas = summary;
                    atlasMenuOpen = false;
                  "
                >
                  <FIcon
                    :name="
                      targetAtlas && sameDocument(summary, targetAtlas)
                        ? 'Check'
                        : 'Image'
                    "
                    :size="14"
                    aria-label=""
                  />
                  <span class="menu-label">{{ atlasLabel(summary) }}</span>
                </button>
                <div v-if="docsLoading" class="menu-empty">
                  {{ $t("common.loading") }}
                </div>
                <div v-else-if="!decalAtlases.length" class="menu-empty">
                  {{ $t("studio.raster.noAtlases") }}
                </div>
              </div>
            </FDropdown>
            <div class="scan-row">
              <button
                type="button"
                class="scan-button"
                :disabled="docsLoading"
                @click="scanDocuments(true)"
              >
                <FIcon
                  :name="docsLoading ? 'Loader2' : 'RefreshCw'"
                  :size="12"
                  aria-label=""
                />
                {{ $t("studio.raster.rescan") }}
              </button>
              <span class="scan-hint">{{ $t("studio.raster.scanAllHint") }}</span>
            </div>

            <template v-if="targetAtlas && doc">
              <label class="form-field">
                <span>{{ $t("studio.raster.decalEntryId") }}</span>
                <input
                  v-model="decalEntryId"
                  type="text"
                  class="form-input mono"
                  placeholder="0x…"
                />
              </label>
              <label class="form-field">
                <span>{{ $t("studio.raster.decalAspectRatio") }}</span>
                <input
                  v-model.number="decalAspectRatio"
                  type="number"
                  min="0.01"
                  step="0.01"
                  class="form-input"
                />
              </label>
              <div class="form-field">
                <span>{{ $t("studio.raster.decalColors") }}</span>
                <div class="color-row">
                  <FColorPicker
                    v-for="(color, slot) in decalColors"
                    :key="slot"
                    v-model="decalColors[slot]"
                    :size="16"
                    :title="$t('studio.raster.decalColorN', { n: slot + 1 })"
                  />
                </div>
              </div>
              <label class="form-check">
                <input v-model="decalReplace" type="checkbox" />
                <span>{{ $t("studio.raster.decalReplace") }}</span>
              </label>
              <label v-if="decalReplace" class="form-field">
                <span>{{ $t("studio.raster.decalReplaceId") }}</span>
                <input
                  v-model="decalReplaceInstance"
                  type="text"
                  class="form-input mono"
                  placeholder="0x…"
                />
              </label>
              <button
                class="primary-button wide"
                type="button"
                :disabled="registering"
                @click="registerDecal"
              >
                <FIcon
                  :name="registering ? 'Loader2' : 'Upload'"
                  :size="14"
                  aria-label=""
                />
                {{ $t("studio.raster.decalRegister") }}
              </button>
              <p class="source-hint">
                {{ $t("studio.raster.decalVerifyHint") }}
              </p>
            </template>
            <p v-else-if="!doc" class="source-hint">
              {{ $t("studio.raster.decalNeedDoc") }}
            </p>
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

            <FDropdown v-model:open="lotMenuOpen" :width="340">
              <template #trigger>
                <button type="button" class="package-trigger">
                  <span>
                    {{
                      targetLot
                        ? lotLabel(targetLot.summary)
                        : $t("studio.raster.pickTargetLot")
                    }}
                  </span>
                  <FIcon name="ChevronDown" :size="12" aria-label="" />
                </button>
              </template>
              <div class="menu-scroll">
                <button
                  v-for="summary in lotDocuments.slice(0, LOT_MENU_LIMIT)"
                  :key="`${summary.packageId}:${hexKey(summary.tgi)}`"
                  type="button"
                  @click="chooseTargetLot(summary)"
                >
                  <FIcon
                    :name="
                      targetLot && sameDocument(summary, targetLot.summary)
                        ? 'Check'
                        : 'Package'
                    "
                    :size="14"
                    aria-label=""
                  />
                  <span class="menu-label">{{ lotLabel(summary) }}</span>
                </button>
                <div v-if="docsLoading" class="menu-empty">
                  {{ $t("common.loading") }}
                </div>
                <div
                  v-else-if="!lotDocuments.length"
                  class="menu-empty"
                >
                  {{ $t("studio.raster.noLots") }}
                </div>
                <div
                  v-else-if="lotDocuments.length > LOT_MENU_LIMIT"
                  class="menu-empty"
                >
                  {{
                    $t("studio.raster.lotMenuCapped", {
                      total: lotDocuments.length,
                      limit: LOT_MENU_LIMIT,
                    })
                  }}
                </div>
              </div>
            </FDropdown>
            <div class="scan-row">
              <button
                type="button"
                class="scan-button"
                :disabled="docsLoading"
                @click="scanDocuments(true)"
              >
                <FIcon
                  :name="docsLoading ? 'Loader2' : 'RefreshCw'"
                  :size="12"
                  aria-label=""
                />
                {{ $t("studio.raster.rescan") }}
              </button>
              <span class="scan-hint">{{ $t("studio.raster.scanAllHint") }}</span>
            </div>

            <!-- 新建 Lot：不需要覆盖目标，直接从画布生成最小 Lot 包 -->
            <div class="new-lot">
              <button
                type="button"
                class="scan-button"
                @click="newLotOpen = !newLotOpen"
              >
                <FIcon
                  :name="newLotOpen ? 'ChevronUp' : 'Plus'"
                  :size="12"
                  aria-label=""
                />
                {{ $t("studio.raster.newLot") }}
              </button>
              <template v-if="newLotOpen">
                <p class="source-meta">
                  {{
                    $t("studio.raster.newLotSizeLabel", {
                      w: doc?.width ?? 0,
                      h: doc?.height ?? 0,
                      wm: ((doc?.width ?? 0) * 0.75).toFixed(2),
                      hm: ((doc?.height ?? 0) * 0.75).toFixed(2),
                    })
                  }}
                </p>
                <div class="form-field">
                  <span>{{ $t("studio.raster.newLotColorsLabel") }}</span>
                  <div
                    v-for="(color, slot) in newLotColors"
                    :key="slot"
                    class="new-lot-row"
                  >
                    <FColorPicker
                      v-model="color.hex"
                      :size="16"
                      :title="$t('studio.raster.newLotColorN', { n: slot + 1 })"
                    />
                    <span class="new-lot-channel">
                      {{
                        $t("studio.raster.newLotColorN", {
                          n: slot + 1,
                          channel: ["R", "G", "B", "A"][slot],
                        })
                      }}
                    </span>
                    <label class="new-lot-tile">
                      <span>{{ $t("studio.raster.newLotTile") }}</span>
                      <input
                        v-model.number="color.tile"
                        type="number"
                        min="0"
                        max="15"
                        class="form-input tile-input"
                      />
                    </label>
                  </div>
                </div>
                <button
                  class="primary-button wide"
                  type="button"
                  :disabled="creatingLot || !doc"
                  @click="createLot"
                >
                  <FIcon
                    :name="creatingLot ? 'Loader2' : 'Save'"
                    :size="14"
                    aria-label=""
                  />
                  {{ $t("studio.raster.createLot") }}
                </button>
                <p class="source-hint">{{ $t("studio.raster.newLotHint") }}</p>
              </template>
            </div>
            <p v-if="targetLot" class="source-meta target-hint">
              <FIcon name="MapPin" :size="12" aria-label="" />
              {{
                $t("studio.raster.targetLotHint", {
                  size: `${targetLot.maskWidth}×${targetLot.maskHeight}`,
                })
              }}
            </p>

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
.preview-card-head {
  align-items: center;
  display: flex;
  gap: 6px;
  justify-content: space-between;
}
.preview-card-head h3 {
  margin: 0;
}
.sheet-open {
  align-items: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 24px;
  padding: 0 6px;
}
.sheet-open:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.preview-sheet {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  padding: 16px;
}
.preview-sheet-head {
  align-items: flex-start;
  display: flex;
  gap: 10px;
  justify-content: space-between;
}
.preview-sheet-toolbar {
  align-items: center;
  display: flex;
  gap: 6px;
}
/* property editor 精细渲染切换的同款分段按钮（默认/开启材质）。 */
.render-mode {
  display: inline-flex;
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
.zoom-option {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  min-height: 24px;
  padding: 0 10px;
}
.zoom-option:hover {
  color: var(--foreground);
}
.zoom-option.active {
  background: var(--surface-hover);
  color: var(--foreground);
}
.preview-sheet-stage {
  align-items: center;
  background-color: var(--surface-elevated);
  background-size: 16px 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  flex: 1;
  justify-content: center;
  min-height: 0;
  overflow: auto;
  padding: 12px;
}
.preview-sheet-stage img {
  image-rendering: pixelated;
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
.menu-scroll {
  display: grid;
  gap: 2px;
  max-height: 320px;
  overflow-y: auto;
  overscroll-behavior: contain;
}
.menu-scroll .menu-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-align: start;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.menu-empty {
  color: var(--muted-foreground);
  font-size: 12px;
  padding: 8px 10px;
  text-align: center;
}
.target-hint {
  align-items: center;
  display: flex;
  gap: 6px;
}
.form-field {
  display: grid;
  gap: 4px;
}
.form-field > span {
  color: var(--muted-foreground);
  font-size: 11.5px;
}
.form-input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  min-height: 28px;
  padding: 0 8px;
  width: 100%;
}
.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
.color-row {
  display: flex;
  gap: 6px;
}
/* FColorPicker 的 trigger 是 width:100%，在 flex 行里会被压扁——给固定宽。 */
.color-row :deep(.color-trigger) {
  flex: none;
  justify-content: center;
  width: 26px;
}
.form-check {
  align-items: center;
  color: var(--foreground);
  display: flex;
  font-size: 12.5px;
  gap: 6px;
}
.size-row {
  align-items: center;
  display: flex;
  gap: 6px;
}
.size-row .form-input {
  width: 72px;
}
.size-x {
  color: var(--muted-foreground);
}
.lock-aspect {
  font-size: 11.5px;
  margin-left: auto;
  white-space: nowrap;
}
.preset-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.preset-button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  min-height: 24px;
  padding: 0 8px;
}
.preset-button:hover {
  border-color: color-mix(in srgb, var(--primary) 55%, var(--border));
  color: var(--primary);
}
.source-hint.warning {
  color: var(--warning);
}
.scan-row {
  align-items: center;
  display: flex;
  gap: 8px;
}
.scan-button {
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
  min-height: 26px;
  padding: 0 10px;
}
.scan-button:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--primary) 55%, var(--border));
  color: var(--primary);
}
.scan-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.scan-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.new-lot {
  border-top: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-top: 10px;
}
.new-lot-row {
  align-items: center;
  display: flex;
  gap: 8px;
}
.new-lot-channel {
  color: var(--muted-foreground);
  font-size: 11.5px;
}
.new-lot-tile {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 11.5px;
  gap: 4px;
  margin-left: auto;
}
.tile-input {
  width: 52px;
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
