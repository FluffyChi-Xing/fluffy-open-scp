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

/**
 * Lot 地表渲染模拟（示意）：量化胜出色 × 白底铺装 + 未覆盖区草地格。
 * 不含 LotColor 染色与 Lot Textures 图集材质——仅表达通道结构语义。
 */
export function simulateLotDataUrl(
  doc: RasterDocument,
  grassTileUrl: string | null,
  tileSize = 32,
): string {
  const canvas = document.createElement("canvas");
  canvas.width = doc.width;
  canvas.height = doc.height;
  const context = canvas.getContext("2d");
  if (!context) return "";
  const quantized = quantizePixels(doc);
  const image = context.createImageData(doc.width, doc.height);
  image.data.set(quantized);
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
