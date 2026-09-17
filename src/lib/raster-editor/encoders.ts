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
