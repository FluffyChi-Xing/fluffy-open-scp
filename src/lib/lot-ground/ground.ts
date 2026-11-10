import type { Rect } from "@/lib/raster-editor/document";
import { lotRegion } from "@/lib/raster-editor/lotPaint";
import type { Rgba } from "@/lib/raster-editor/tools";

/**
 * Lot 地面的用户模型：一块底图（图集格，不可染色）+ 最多 4 种绘制材质
 * （图集格 + 绑定颜色）。材质槽下标 = LotMask 通道（R/G/B/A），只在本模块
 * 内部出现；界面只呈现"已用材质"。
 */
export const LOT_METERS_PER_PIXEL = 0.75;
export const LOT_SLOT_COUNT = 4;
const MAX_MASK_SIDE = 4096;

export type Rgb = [number, number, number];
export interface LotMaterial {
  /** 共享 4×4 图集格（0-15），引擎写入 LotColor.A。 */
  pattern: number;
  /** 平色（sRGB 字节），引擎写入 LotColor.RGB。 */
  color: Rgb;
}
/** 长度恒为 4；下标即通道，null = 未使用。 */
export type LotSlots = (LotMaterial | null)[];

export const DEFAULT_MATERIAL_COLOR: Rgb = [140, 140, 140];
const UNUSED_SLOT_COLOR: Rgb = [128, 128, 128];

export function maskSizeForLot(
  size: readonly [number, number],
  metersPerPixel = LOT_METERS_PER_PIXEL,
): { width: number; height: number } {
  const side = (meters: number) =>
    Math.min(MAX_MASK_SIDE, Math.max(1, Math.round(meters / metersPerPixel)));
  return { width: side(size[0]), height: side(size[1]) };
}

export function lotSizeForMask(
  width: number,
  height: number,
  metersPerPixel = LOT_METERS_PER_PIXEL,
): [number, number] {
  return [width * metersPerPixel, height * metersPerPixel];
}

/** 用户输入的米制尺寸仅在与遮罩像素一致时生效，否则交给后端按 px × 0.75 推导。 */
export function lotSizeForImage(
  size: readonly number[] | undefined,
  width: number,
  height: number,
  metersPerPixel = LOT_METERS_PER_PIXEL,
): [number, number] | undefined {
  if (!size || size.length !== 2) return undefined;
  const mask = maskSizeForLot([size[0], size[1]], metersPerPixel);
  return mask.width === width && mask.height === height ? [size[0], size[1]] : undefined;
}

/** 材质画笔：只写该槽通道（one-hot），引擎裁决下永远按用户意图归属。 */
export function slotPaint(slot: number): Rgba {
  return [0, 1, 2, 3].map((channel) => (channel === slot ? 255 : 0)) as unknown as Rgba;
}

export function firstFreeSlot(slots: LotSlots): number {
  return slots.findIndex((slot) => !slot);
}

export function usedSlots(slots: LotSlots): boolean[] {
  return slots.map(Boolean);
}

/** 每个通道按引擎 A > B > G > R 裁决后实际占有的像素数。 */
export function slotCoverage(
  pixels: Uint8ClampedArray,
  widths: readonly number[] = [],
): number[] {
  const counts = [0, 0, 0, 0];
  for (let at = 0; at < pixels.length; at += 4) {
    const region = lotRegion(
      [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]],
      widths,
    );
    if (region >= 0) counts[Math.floor(region / 2)] += 1;
  }
  return counts;
}

/** LotColor1-4（RGB + A=图集格）→ 材质槽；used 决定哪些槽对用户可见。 */
export function slotsFromLot(
  colors: readonly (readonly number[])[],
  used: readonly boolean[],
): LotSlots {
  return Array.from({ length: LOT_SLOT_COUNT }, (_, slot) => {
    const color = colors[slot];
    if (!used[slot] || !color) return null;
    return {
      pattern: clampPattern(color[3] ?? 0),
      color: [color[0] ?? 0, color[1] ?? 0, color[2] ?? 0],
    };
  });
}

/** 材质槽 → LotColor1-4；未使用的槽保留原值，避免改写来源 Lot 的数据。 */
export function lotColorsFromSlots(
  slots: LotSlots,
  previous: readonly (readonly number[])[],
): [number, number, number, number][] {
  return Array.from({ length: LOT_SLOT_COUNT }, (_, slot) => {
    const material = slots[slot];
    if (material) return [...material.color, clampPattern(material.pattern)];
    const old = previous[slot] ?? [...UNUSED_SLOT_COLOR, 0];
    return [old[0] ?? 0, old[1] ?? 0, old[2] ?? 0, clampPattern(old[3] ?? 0)];
  });
}

export interface GroundDisplayOptions {
  width: number;
  height: number;
  /** 图集格在画布上的平铺周期（像素）。 */
  tilePx: readonly [number, number];
  /** 每槽图集格像素；缺省则回退为平色。 */
  cells?: readonly (ImageData | null | undefined)[];
}

/**
 * 画布显示：按引擎裁决给每个像素涂上胜出材质；有图集格时按底图同口径
 * 平铺纹理并用绑定色轻度着色，避免「缩略图有纹、画布纯色」的体验落差。
 * 未覆盖区透明，由画布背后的底图格平铺显示。边框带用边框色（缺省同主色）。
 */
export function groundDisplayPixels(
  pixels: Uint8ClampedArray,
  slots: LotSlots,
  widths: readonly number[] = [],
  borders: readonly (readonly number[] | undefined)[] = [],
  options?: GroundDisplayOptions,
): Uint8ClampedArray<ArrayBuffer> {
  const output = new Uint8ClampedArray(pixels.length);
  const width = options?.width ?? 0;
  const tileW = Math.max(1, options?.tilePx[0] ?? 1);
  const tileH = Math.max(1, options?.tilePx[1] ?? 1);
  for (let at = 0, index = 0; at < pixels.length; at += 4, index += 1) {
    const region = lotRegion(
      [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]],
      widths,
    );
    if (region < 0) continue;
    const channel = Math.floor(region / 2);
    const main = slots[channel]?.color ?? UNUSED_SLOT_COLOR;
    const color = region % 2 ? (borders[channel] ?? main) : main;
    const cell = region % 2 ? null : options?.cells?.[channel];
    if (cell && width > 0) {
      const x = index % width;
      const y = Math.floor(index / width);
      const sample = sampleTiledCell(cell, x, y, tileW, tileH);
      output[at] = tintChannel(sample[0], color[0] ?? 128);
      output[at + 1] = tintChannel(sample[1], color[1] ?? 128);
      output[at + 2] = tintChannel(sample[2], color[2] ?? 128);
    } else {
      output[at] = color[0] ?? 0;
      output[at + 1] = color[1] ?? 0;
      output[at + 2] = color[2] ?? 0;
    }
    output[at + 3] = 255;
  }
  return output;
}

function sampleTiledCell(
  cell: ImageData,
  x: number,
  y: number,
  tileW: number,
  tileH: number,
): [number, number, number] {
  const u = (((x % tileW) + tileW) % tileW) / tileW;
  const v = (((y % tileH) + tileH) % tileH) / tileH;
  const cx = Math.min(cell.width - 1, Math.floor(u * cell.width));
  const cy = Math.min(cell.height - 1, Math.floor(v * cell.height));
  const at = (cy * cell.width + cx) * 4;
  return [cell.data[at], cell.data[at + 1], cell.data[at + 2]];
}

/** 中性色 128 近似原贴图；偏色时保留图集纹理。 */
function tintChannel(sample: number, tint: number): number {
  return Math.max(0, Math.min(255, Math.round((sample * tint) / 128)));
}

/** 清空某槽通道（移除材质）；返回改动包围盒，无改动返回 null。 */
export function clearSlotPixels(
  pixels: Uint8ClampedArray,
  width: number,
  slot: number,
): Rect | null {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -1;
  let maxY = -1;
  for (let at = slot, index = 0; at < pixels.length; at += 4, index += 1) {
    if (!pixels[at]) continue;
    pixels[at] = 0;
    const x = index % width;
    const y = Math.floor(index / width);
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x);
    maxY = Math.max(maxY, y);
  }
  return maxX < 0 ? null : { x: minX, y: minY, w: maxX - minX + 1, h: maxY - minY + 1 };
}

/** 最近邻重采样：地块尺寸变化时保持布局形状，不产生混合通道值。 */
export function resampleNearest(
  pixels: Uint8ClampedArray,
  width: number,
  height: number,
  nextWidth: number,
  nextHeight: number,
): Uint8ClampedArray<ArrayBuffer> {
  const output = new Uint8ClampedArray(nextWidth * nextHeight * 4);
  for (let y = 0; y < nextHeight; y += 1) {
    const sourceY = Math.min(height - 1, Math.floor(((y + 0.5) * height) / nextHeight));
    for (let x = 0; x < nextWidth; x += 1) {
      const sourceX = Math.min(width - 1, Math.floor(((x + 0.5) * width) / nextWidth));
      const from = (sourceY * width + sourceX) * 4;
      output.set(pixels.subarray(from, from + 4), (y * nextWidth + x) * 4);
    }
  }
  return output;
}

function clampPattern(value: number): number {
  return Math.max(0, Math.min(15, Math.round(value) || 0));
}
