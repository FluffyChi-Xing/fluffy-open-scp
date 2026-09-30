import type { RasterDocument } from "./document";

/**
 * raster-editor 的编码/解码辅助：像素缓冲 ↔ ImageData / base64。
 * （raster 容器字节 ↔ RGBA 的编解码在后端 rw4::raster 完成。）
 */

/** 文档像素 → ImageData（画布 putImageData 用）。返回副本，调用方可自由改写。 */
export function toImageData(doc: RasterDocument): ImageData {
  return new ImageData(
    new Uint8ClampedArray(doc.pixels),
    doc.width,
    doc.height,
  );
}

export function rgbaToBase64(rgba: Uint8ClampedArray): string {
  let binary = "";
  const chunk = 0x8000;
  for (let index = 0; index < rgba.length; index += chunk) {
    binary += String.fromCharCode(...rgba.subarray(index, index + chunk));
  }
  return btoa(binary);
}

export function base64ToRgba(base64: string): Uint8ClampedArray {
  const binary = atob(base64);
  const out = new Uint8ClampedArray(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    out[index] = binary.charCodeAt(index);
  }
  return out;
}

/**
 * 原始 RGBA(base64) → PNG data URL（画布中转）。
 * 后端 read_image_rgba 下发的是裸 RGBA 字节而非 PNG，直接拼
 * `data:image/png` 会静默解码失败——外部导入预览必须走这里。
 */
export function rgbaBase64ToPngDataUrl(
  base64: string,
  width: number,
  height: number,
): string {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return "";
  const image = context.createImageData(width, height);
  image.data.set(base64ToRgba(base64));
  context.putImageData(image, 0, 0);
  return canvas.toDataURL("image/png");
}

/** 等比缩放目标尺寸计算：长边贴到 max，短边按比例取整（≥1）。 */
export function fitSizeWithin(
  width: number,
  height: number,
  max: number,
): { width: number; height: number } {
  if (width <= max && height <= max) return { width, height };
  const scale = max / Math.max(width, height);
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}

/** RGBA 像素缩放到目标尺寸（画布双线性，高质量）。 */
export function scaleRgba(
  rgba: Uint8ClampedArray,
  width: number,
  height: number,
  targetWidth: number,
  targetHeight: number,
): Uint8ClampedArray {
  if (width === targetWidth && height === targetHeight) return rgba;
  const source = document.createElement("canvas");
  source.width = width;
  source.height = height;
  const sourceContext = source.getContext("2d");
  const target = document.createElement("canvas");
  target.width = targetWidth;
  target.height = targetHeight;
  const targetContext = target.getContext("2d");
  if (!sourceContext || !targetContext) return rgba;
  sourceContext.putImageData(
    new ImageData(new Uint8ClampedArray(rgba), width, height),
    0,
    0,
  );
  targetContext.imageSmoothingEnabled = true;
  targetContext.imageSmoothingQuality = "high";
  targetContext.drawImage(
    source,
    0,
    0,
    width,
    height,
    0,
    0,
    targetWidth,
    targetHeight,
  );
  return targetContext.getImageData(0, 0, targetWidth, targetHeight).data;
}

/**
 * LotMask/Decal 量化调色板（SCP 惯例下标序 [color4, color3, color2, color1]
 * = 蓝 / 绿 / 红 / 黑）。
 */
export const QUANTIZED_PALETTE: readonly (readonly [number, number, number])[] =
  [
    [0, 0, 255],
    [0, 255, 0],
    [255, 0, 0],
    [0, 0, 0],
  ];

/**
 * 四层量化视图：每像素按优先级 A > R > G > B 取首个 ≥128 的通道，
 * 映射到量化调色板输出不透明像素；全部未达阈值输出透明。
 * 与后端 rw4 decode_lot_mask_rgba 同口径——即游戏实际使用的清晰结构。
 */
export function quantizePixels(doc: RasterDocument) {
  const out = new Uint8ClampedArray(doc.width * doc.height * 4);
  for (let index = 0; index < doc.width * doc.height; index += 1) {
    const base = index * 4;
    const channels = [
      doc.pixels[base + 3],
      doc.pixels[base],
      doc.pixels[base + 1],
      doc.pixels[base + 2],
    ];
    for (let layer = 0; layer < 4; layer += 1) {
      if (channels[layer] >= 128) {
        const [r, g, b] = QUANTIZED_PALETTE[layer];
        out[base] = r;
        out[base + 1] = g;
        out[base + 2] = b;
        out[base + 3] = 255;
        break;
      }
    }
  }
  return out;
}

export function quantizeToImageData(doc: RasterDocument): ImageData {
  return new ImageData(quantizePixels(doc), doc.width, doc.height);
}

/**
 * Lot/Decal 四层"通道画笔"：lot mask 的每个通道是 LotColor1-4 的材质层
 * 权重（R→C1 路面 / G→C2 铺装 / B→C3 草坪 / A→C4 标线，颜色相加合成），
 * 画"任意 RGB"会同时激活多层，因此语义图只允许整层整笔地画。
 * paint = 该层满权重的原始像素值；display = 量化视图里该层的显示色
 * （与 QUANTIZED_PALETTE 的 A > R > G > B 优先级口径一致）。
 */
export interface LotLayer {
  /** 写入文档的原始像素（仅目标通道满权重）。 */
  paint: readonly [number, number, number, number];
  /** 量化视图显示色（CSS hex）。 */
  display: string;
  /** 通道名（LotColor 下标语义）。 */
  channel: "A" | "R" | "G" | "B";
}

export const LOT_LAYERS: readonly LotLayer[] = [
  { paint: [0, 0, 0, 255], display: "#0000ff", channel: "A" },
  { paint: [255, 0, 0, 0], display: "#00ff00", channel: "R" },
  { paint: [0, 255, 0, 0], display: "#ff0000", channel: "G" },
  { paint: [0, 0, 255, 0], display: "#000000", channel: "B" },
];

/** 按 A > R > G > B 优先级把原始像素吸附到层下标（取色器用）；全未命中返回 null。 */
export function layerOfPixel(
  rgba: readonly [number, number, number, number],
): number | null {
  const channels = [rgba[3], rgba[0], rgba[1], rgba[2]];
  for (let layer = 0; layer < 4; layer += 1) {
    if (channels[layer] >= 128) return layer;
  }
  return null;
}

/** SCP 缺省 LotColor1-4（黑/红/绿/蓝；仅 RGB 染色分量，A=tile 索引不参与）。 */
export const FALLBACK_LOT_DYES: readonly (readonly [number, number, number])[] = [
  [0, 0, 0],
  [255, 0, 0],
  [0, 255, 0],
  [0, 0, 255],
];

/**
 * Lot 地表渲染模拟（引擎口径）：四通道按游戏映射染色——R→LotColor1、
 * G→LotColor2、B→LotColor3、A→LotColor4（优先级 A > R > G > B，阈值 128，
 * 与 rw4 decode_lot_mask_rgba 同口径），未覆盖区铺草地格。染料色 = 覆盖
 * 目标 lot 的 LotColor1-4 授权（sRGB 字节），未载入目标时用 SCP 缺省色。
 * 不含 Lot Textures 图集材质与平铺周期——以属性授权为准。
 */
export function simulateLotDataUrl(
  doc: RasterDocument,
  grassTileUrl: string | null,
  dyeColors: readonly (readonly [number, number, number])[] = FALLBACK_LOT_DYES,
  tileSize = 32,
): string {
  const canvas = document.createElement("canvas");
  canvas.width = doc.width;
  canvas.height = doc.height;
  const context = canvas.getContext("2d");
  if (!context) return "";
  // 通道序 [A,R,G,B] → LotColor 下标 [3,0,1,2]。
  const channelToDye = [3, 0, 1, 2];
  const image = context.createImageData(doc.width, doc.height);
  for (let index = 0; index < doc.width * doc.height; index += 1) {
    const base = index * 4;
    const channels = [
      doc.pixels[base + 3],
      doc.pixels[base],
      doc.pixels[base + 1],
      doc.pixels[base + 2],
    ];
    for (let layer = 0; layer < 4; layer += 1) {
      if (channels[layer] >= 128) {
        const [r, g, b] = dyeColors[channelToDye[layer]] ?? FALLBACK_LOT_DYES[channelToDye[layer]];
        image.data[base] = r;
        image.data[base + 1] = g;
        image.data[base + 2] = b;
        image.data[base + 3] = 255;
        break;
      }
    }
  }
  const temp = document.createElement("canvas");
  temp.width = doc.width;
  temp.height = doc.height;
  temp.getContext("2d")?.putImageData(image, 0, 0);
  context.imageSmoothingEnabled = false;
  context.drawImage(temp, 0, 0);
  if (grassTileUrl) {
    const tile = document.createElement("canvas");
    tile.width = tileSize;
    tile.height = tileSize;
    const tileContext = tile.getContext("2d");
    const grass = new Image();
    grass.src = grassTileUrl;
    // 同步绘制依赖已完成加载的缓存；未加载时跳过（下一次刷新补上）
    if (tileContext && grass.complete && grass.naturalWidth > 0) {
      tileContext.drawImage(grass, 0, 0, tileSize, tileSize);
      context.globalCompositeOperation = "destination-over";
      const pattern = context.createPattern(tile, "repeat");
      if (pattern) {
        context.fillStyle = pattern;
        context.fillRect(0, 0, doc.width, doc.height);
      }
      context.globalCompositeOperation = "source-over";
    }
  }
  return canvas.toDataURL("image/png");
}

/**
 * Decal 渲染模拟（示意）：量化四色印花叠在棋盘透明底上。
 */
export function simulateDecalDataUrl(doc: RasterDocument, cell = 12): string {
  const canvas = document.createElement("canvas");
  canvas.width = doc.width;
  canvas.height = doc.height;
  const context = canvas.getContext("2d");
  if (!context) return "";
  const image = context.createImageData(doc.width, doc.height);
  image.data.set(quantizePixels(doc));
  const temp = document.createElement("canvas");
  temp.width = doc.width;
  temp.height = doc.height;
  temp.getContext("2d")?.putImageData(image, 0, 0);
  context.imageSmoothingEnabled = false;
  context.drawImage(temp, 0, 0);
  void cell;
  return canvas.toDataURL("image/png");
}

/** 生成棋盘格透明底图案（data URL，供画布容器 CSS 平铺）。 */
export function makeCheckerboard(
  size = 8,
  light = "#2a2d3a",
  dark = "#22242e",
): string {
  const canvas = document.createElement("canvas");
  canvas.width = size * 2;
  canvas.height = size * 2;
  const context = canvas.getContext("2d");
  if (!context) return "";
  context.fillStyle = light;
  context.fillRect(0, 0, size * 2, size * 2);
  context.fillStyle = dark;
  context.fillRect(0, 0, size, size);
  context.fillRect(size, size, size, size);
  return canvas.toDataURL();
}

// ── Lot 材质合成（引擎语义，property editor refinedGround 同口径的 2D 版） ──

const groundTextureUrls = import.meta.glob<string>(
  "../../assets/ground/*.png",
  { eager: true, import: "default", query: "?url" },
) as unknown as Record<string, string>;

const groundTileUrls = new Map<number, string>();
for (const [path, url] of Object.entries(groundTextureUrls)) {
  const name = path.slice(path.lastIndexOf("/") + 1, -4);
  const index = Number.parseInt(name, 10);
  if (Number.isInteger(index)) groundTileUrls.set(index, url);
}

function loadImage(src: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => resolve(null);
    image.src = src;
  });
}

/** 本地占位 tile（src/assets/ground，格号 = 文件名；surface 图集缺失回退）。 */
async function localTile(index: number): Promise<ImageData | null> {
  const url = groundTileUrls.get(((index % 16) + 16) % 16);
  if (!url) return null;
  const image = await loadImage(url);
  if (!image) return null;
  const canvas = document.createElement("canvas");
  canvas.width = image.naturalWidth;
  canvas.height = image.naturalHeight;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.drawImage(image, 0, 0);
  return context.getImageData(0, 0, canvas.width, canvas.height);
}

/**
 * Lot 地表材质合成（2D sheet 版，与 property editor refinedGround /
 * lot_composite.rs 校准口径同源，docs/blog/raster-lot-rendering.md §3）：
 *  1. 选区：mask 通道权重 > 0.5−bw 硬阈值，引擎优先级瀑布 A边框 > A主色 >
 *     B边框 > … > R主色（w→z→y→x）；边框带 = 权重 ∈ (0.5−bw, 0.5+bw]；
 *  2. 反照率：胜者**平色**直出（主区 LotColor / 边框带 LotBorderColor，
 *     后端已 linear→sRGB）——引擎覆盖区不采样漫反射（tile×tint 双重变暗
 *     已证伪）；LotColor 缺失即引擎回退调色板（黑/红/绿/蓝）照常参与；
 *  3. 图案质感：法线图集（主区格号 = LotColor.A / 边框带 = LotBorderColor.A）
 *     按 (u−0.5)·tiles 相位平铺，坡度明暗直接烘焙进反照率（2D sheet 无光照
 *     上下文，复刻 lot_composite pattern 模式的 lotCalcLighting 近似，
 *     输出与 output/lot_hires/*_hires_pattern.png 同口径）；
 *  4. 未覆盖区：底图格（数据驱动 baseTile 索引）整格拉伸铺满，U 轴镜像
 *     （§5b 图集 U 轴与 mask 列序相反），无图案光照。
 * 4× 超采样与 refinedGround 同参。
 */
export async function composeLotMaterialDataUrl(options: {
  doc: RasterDocument;
  /** LC1-4（sRGB RGB + A = 图案/法线格号 0-15）。 */
  lotColors: [number, number, number, number][];
  /** LotBorderColor1-4 的 sRGB RGB。 */
  lotBorderColors?: [number, number, number][];
  /** borderWidth1-4（边框带半宽）；缺省全 0 = 无边框带。 */
  lotBorderWidths?: number[];
  /** 边框带图案索引（LotBorderColor.A）。 */
  lotBorderPatternIndices?: number[];
  /** 底图格索引（后端三级来源解析；缺省 8）。 */
  baseTileIndex?: number;
  /** Lot Textures 图集 PNG data URL；null = 本地占位 tile。 */
  surfaceUrl: string | null;
  /** 法线图案图集 PNG data URL；null = 无图案光照（纯平色）。 */
  normalAtlasUrl?: string | null;
  /** 地面贴图周期（米/格）；null 回退实测拟合常量。 */
  tilePeriod?: [number, number] | null;
  /** 地面尺寸（米）；缺省 = mask px × 0.75。 */
  lotSize?: [number, number] | null;
}): Promise<string> {
  const { doc } = options;
  const [surface, normalAtlas] = await Promise.all([
    loadImageData(options.surfaceUrl),
    options.normalAtlasUrl ? loadImageData(options.normalAtlasUrl) : null,
  ]);
  const useSurface = Boolean(surface && surface.width >= 4 && surface.height >= 4);
  const tileW = useSurface ? Math.floor(surface!.width / 4) : 0;
  const tileH = useSurface ? Math.floor(surface!.height / 4) : 0;
  const copyRegion = (index: number): ImageData | null => {
    if (!useSurface) return null;
    const out = new ImageData(tileW, tileH);
    const originX = (index % 4) * tileW;
    const originY = Math.floor(index / 4) * tileH;
    for (let y = 0; y < tileH; y += 1) {
      const srcRow = ((originY + y) * surface!.width + originX) * 4;
      out.data.set(
        surface!.data.subarray(srcRow, srcRow + tileW * 4),
        y * tileW * 4,
      );
    }
    return out;
  };
  // 底图格：数据驱动索引（缺省草地 8），surface 缺失回退本地占位 tile。
  const baseTileIndex = (options.baseTileIndex ?? 8) % 16;
  const baseTile = useSurface ? copyRegion(baseTileIndex) : await localTile(baseTileIndex);
  // 图案格（法线图集 4×4），按格号惰性切片。
  const useNormal = Boolean(
    normalAtlas && normalAtlas.width >= 4 && normalAtlas.height >= 4,
  );
  const normalCellW = useNormal ? Math.floor(normalAtlas!.width / 4) : 0;
  const normalCellH = useNormal ? Math.floor(normalAtlas!.height / 4) : 0;
  const patternCells = new Map<number, ImageData | null>();
  const patternCell = (index: number): ImageData | null => {
    if (!useNormal) return null;
    const cell = ((index % 16) + 16) % 16;
    if (!patternCells.has(cell)) {
      patternCells.set(cell, copyNormalCell(normalAtlas!, cell, normalCellW, normalCellH));
    }
    return patternCells.get(cell) ?? null;
  };

  const borderWidths =
    options.lotBorderWidths && options.lotBorderWidths.length === 4
      ? options.lotBorderWidths
      : [0, 0, 0, 0];
  const borderIndices =
    options.lotBorderPatternIndices && options.lotBorderPatternIndices.length === 4
      ? options.lotBorderPatternIndices
      : [0, 0, 0, 0];

  const periodX =
    options.tilePeriod?.[0] && options.tilePeriod[0] > 0
      ? options.tilePeriod[0]
      : 9.6;
  const periodY =
    options.tilePeriod?.[1] && options.tilePeriod[1] > 0
      ? options.tilePeriod[1]
      : 9.6;
  const [lotW, lotH] = options.lotSize ?? [doc.width * 0.75, doc.height * 0.75];
  const tilesX = lotW > 0 ? Math.max(0.1, lotW / periodX) : 1;
  const tilesY = lotH > 0 ? Math.max(0.1, lotH / periodY) : 1;

  // 4× 超采样：mask 权重双线性插值 + 平色/底图直出（refinedGround 同参）。
  const scale = Math.min(4, Math.max(1, Math.floor(1024 / Math.max(doc.width, doc.height))));
  const outW = doc.width * scale;
  const outH = doc.height * scale;
  const canvas = document.createElement("canvas");
  canvas.width = outW;
  canvas.height = outH;
  const context = canvas.getContext("2d");
  if (!context) return "";

  /** 画布 UV → doc 原始通道权重（双线性，= 引擎 GPU 采样口径）。 */
  const sampleWeights = (u: number, v: number): [number, number, number, number] => {
    const fx = Math.min(Math.max(u * doc.width - 0.5, 0), doc.width - 1);
    const fy = Math.min(Math.max(v * doc.height - 0.5, 0), doc.height - 1);
    const x0 = Math.floor(fx);
    const y0 = Math.floor(fy);
    const x1 = Math.min(x0 + 1, doc.width - 1);
    const y1 = Math.min(y0 + 1, doc.height - 1);
    const tx = fx - x0;
    const ty = fy - y0;
    const rowA = y0 * doc.width;
    const rowB = y1 * doc.width;
    const out: [number, number, number, number] = [0, 0, 0, 0];
    for (let c = 0; c < 4; c += 1) {
      const a = doc.pixels[(rowA + x0) * 4 + c];
      const b = doc.pixels[(rowA + x1) * 4 + c];
      const top = a + (b - a) * tx;
      const cc = doc.pixels[(rowB + x0) * 4 + c];
      const d = doc.pixels[(rowB + x1) * 4 + c];
      const bottom = cc + (d - cc) * tx;
      out[c] = (top + (bottom - top) * ty) / 255;
    }
    return out;
  };
  const pixelAt = (source: ImageData, px: number, py: number): [number, number, number] => {
    const offset = (py * source.width + px) * 4;
    return [source.data[offset], source.data[offset + 1], source.data[offset + 2]];
  };

  const composed = context.createImageData(outW, outH);
  for (let y = 0; y < outH; y += 1) {
    for (let x = 0; x < outW; x += 1) {
      const at = (y * outW + x) * 4;
      const u = (x + 0.5) / outW;
      const v = (y + 0.5) / outH;
      // 引擎优先级瀑布 w→z→y→x = A > B > G > R（addOverlay 逐字）。
      const weights = sampleWeights(u, v);
      let channel = -1;
      let channelIsBorder = false;
      for (const c of [3, 2, 1, 0]) {
        if (weights[c] > 0.5 - borderWidths[c]) {
          channel = c;
          channelIsBorder = weights[c] <= 0.5 + borderWidths[c];
          break;
        }
      }
      if (channel >= 0) {
        const flat = channelIsBorder
          ? options.lotBorderColors?.[channel] ?? [156, 156, 156]
          : options.lotColors[channel];
        const cell = patternCell(
          channelIsBorder ? borderIndices[channel] : options.lotColors[channel][3],
        );
        const shade = cell ? patternShade(cell, u, v, tilesX, tilesY) : 1;
        for (let c = 0; c < 3; c += 1) {
          composed.data[at + c] = linearToSrgbU8(srgbToLinearF32(flat[c]) * shade);
        }
      } else if (baseTile) {
        // 底图格整格拉伸（U 镜像），无图案光照。
        const px = Math.min(baseTile.width - 1, Math.floor((1 - u) * baseTile.width));
        const py = Math.min(baseTile.height - 1, Math.floor(v * baseTile.height));
        const [br, bg, bb] = pixelAt(baseTile, px, py);
        composed.data[at] = br;
        composed.data[at + 1] = bg;
        composed.data[at + 2] = bb;
      } else {
        composed.data[at] = 255;
        composed.data[at + 1] = 255;
        composed.data[at + 2] = 255;
      }
      composed.data[at + 3] = 255;
    }
  }
  context.putImageData(composed, 0, 0);
  return canvas.toDataURL("image/png");
}

/** sRGB 字节 → 线性 0..1（lot_composite 同公式）。 */
function srgbToLinearF32(byte: number): number {
  const srgb = byte / 255;
  return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
}

/** 线性 0..1 → sRGB 字节。 */
function linearToSrgbU8(linear: number): number {
  const clamped = Math.min(1, Math.max(0, Number.isFinite(linear) ? linear : 0));
  const srgb =
    clamped <= 0.0031308 ? 12.92 * clamped : 1.055 * clamped ** (1 / 2.4) - 0.055;
  return Math.round(srgb * 255);
}

/**
 * 图案格法线 → 坡度明暗（lot_composite `shade_of` 同款：平整 = 1.0，
 * 逆光面暗、向光面亮，近似引擎 lotCalcLighting 的图案光照）。
 */
function patternShade(
  cell: ImageData,
  u: number,
  v: number,
  tilesX: number,
  tilesY: number,
): number {
  const LIGHT_X = -0.5;
  const LIGHT_Y = -0.5;
  const STRENGTH = 1.4;
  const px = Math.min(
    cell.width - 1,
    Math.floor(frac01((u - 0.5) * tilesX) * cell.width),
  );
  const py = Math.min(
    cell.height - 1,
    Math.floor(frac01((v - 0.5) * tilesY) * cell.height),
  );
  const offset = (py * cell.width + px) * 4;
  const nx = cell.data[offset] / 127.5 - 1;
  const ny = cell.data[offset + 1] / 127.5 - 1;
  return Math.min(1.45, Math.max(0.55, 1 + STRENGTH * (nx * LIGHT_X + ny * LIGHT_Y)));
}

/** fract 到 [0,1)。 */
function frac01(value: number): number {
  return value - Math.floor(value);
}

/** 从法线图集切第 index 格（4×4 格，行主序）。 */
function copyNormalCell(
  atlas: ImageData,
  index: number,
  cellW: number,
  cellH: number,
): ImageData | null {
  if (cellW < 1 || cellH < 1) return null;
  const out = new ImageData(cellW, cellH);
  const originX = (index % 4) * cellW;
  const originY = Math.floor(index / 4) * cellH;
  for (let y = 0; y < cellH; y += 1) {
    const srcRow = ((originY + y) * atlas.width + originX) * 4;
    out.data.set(atlas.data.subarray(srcRow, srcRow + cellW * 4), y * cellW * 4);
  }
  return out;
}

/** data URL → ImageData（失败 = null）。 */
async function loadImageData(url: string | null | undefined): Promise<ImageData | null> {
  if (!url) return null;
  const image = await loadImage(url);
  if (!image) return null;
  const canvas = document.createElement("canvas");
  canvas.width = image.naturalWidth;
  canvas.height = image.naturalHeight;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.drawImage(image, 0, 0);
  return context.getImageData(0, 0, canvas.width, canvas.height);
}
