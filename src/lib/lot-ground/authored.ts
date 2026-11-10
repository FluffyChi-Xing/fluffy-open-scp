import { base64ToRgba, rgbaToBase64 } from "@/lib/raster-editor/encoders";
import { encodeRgbaPng } from "@/lib/raster-editor/png";
import type { RasterSchema } from "@/lib/raster-editor/schema";
import { composeGroundPixels } from "@/render/resolve/groundCompose";
import { LOT_METERS_PER_PIXEL, LOT_SLOT_COUNT, lotSizeForMask } from "./ground";

/**
 * 用户自绘 Lot 地面的项目内静态存档（assets/lot/authored/）：
 * ground.json = 尺寸与材质，mask.rgba = 遮罩原始 RGBA 字节。
 * 与 Raster 编辑文档（RasterSchema）双向转换，供全屏绘制 sheet 读写。
 */
export const AUTHORED_LOT_DIR = "assets/lot/authored";
export const AUTHORED_LOT_META = `${AUTHORED_LOT_DIR}/ground.json`;
export const AUTHORED_LOT_MASK = `${AUTHORED_LOT_DIR}/mask.rgba`;

type Rgba4 = [number, number, number, number];
type Rgb3 = [number, number, number];

export interface AuthoredLotMeta {
  format: "openscp.lot-ground";
  version: 1;
  name: string;
  size: [number, number];
  width: number;
  height: number;
  metersPerPixel: number;
  baseTile: number;
  tilePeriod: [number, number] | null;
  colors: Rgba4[];
  slots: boolean[];
  borderColors: Rgb3[];
  borderWidths: number[];
  borderPatternIndices: number[];
}

export interface AuthoredLot {
  meta: AuthoredLotMeta;
  /** 遮罩 RGBA 字节 base64（width × height × 4）。 */
  mask: string;
}

const DEFAULT_BORDER: Rgb3 = [156, 156, 156];

function finiteArray(value: unknown, count: number): value is number[] {
  return (
    Array.isArray(value) &&
    value.length === count &&
    value.every((n) => typeof n === "number" && Number.isFinite(n))
  );
}

/** ground.json 校验；不合法返回 null（视为没有自绘地面）。 */
export function parseAuthoredLotMeta(text: string): AuthoredLotMeta | null {
  let value: Partial<AuthoredLotMeta>;
  try {
    value = JSON.parse(text) as Partial<AuthoredLotMeta>;
  } catch {
    return null;
  }
  const ok =
    value.format === "openscp.lot-ground" &&
    value.version === 1 &&
    finiteArray(value.size, 2) &&
    Number.isInteger(value.width) &&
    Number.isInteger(value.height) &&
    (value.width ?? 0) > 0 &&
    (value.height ?? 0) > 0 &&
    typeof value.metersPerPixel === "number" &&
    Number.isInteger(value.baseTile) &&
    Array.isArray(value.colors) &&
    value.colors.length === LOT_SLOT_COUNT &&
    value.colors.every((color) => finiteArray(color, 4)) &&
    Array.isArray(value.slots) &&
    value.slots.length === LOT_SLOT_COUNT &&
    (value.tilePeriod == null || finiteArray(value.tilePeriod, 2));
  if (!ok) return null;
  return {
    ...(value as AuthoredLotMeta),
    name: typeof value.name === "string" ? value.name : "",
    tilePeriod: value.tilePeriod ?? null,
    borderColors:
      Array.isArray(value.borderColors) && value.borderColors.length === LOT_SLOT_COUNT
        ? value.borderColors
        : Array.from({ length: LOT_SLOT_COUNT }, () => [...DEFAULT_BORDER] as Rgb3),
    borderWidths: finiteArray(value.borderWidths, 4) ? value.borderWidths : [0, 0, 0, 0],
    borderPatternIndices: finiteArray(value.borderPatternIndices, 4)
      ? value.borderPatternIndices
      : [0, 0, 0, 0],
  };
}

export function maskMatchesMeta(meta: AuthoredLotMeta, mask: string): boolean {
  return base64ToRgba(mask).length === meta.width * meta.height * 4;
}

/** Raster 编辑文档（lot-mask）→ 自绘地面存档。 */
export function authoredFromSchema(schema: RasterSchema): AuthoredLot {
  if (schema.profile !== "lot-mask" || !schema.image) {
    throw new Error("lot_ground_schema_invalid");
  }
  const { width, height, data } = schema.image;
  const metersPerPixel = schema.sampling.metersPerPixel || LOT_METERS_PER_PIXEL;
  const lot = schema.lot;
  return {
    meta: {
      format: "openscp.lot-ground",
      version: 1,
      name: schema.name,
      size: lot.size ?? lotSizeForMask(width, height, metersPerPixel),
      width,
      height,
      metersPerPixel,
      baseTile: lot.baseTile ?? 8,
      tilePeriod: lot.tilePeriod ?? null,
      colors: lot.colors.map((color) => [...color] as Rgba4),
      slots: lot.slots ? [...lot.slots] : lot.colors.map(() => true),
      borderColors: (lot.borderColors ?? []).length === LOT_SLOT_COUNT
        ? lot.borderColors!.map((color) => [...color] as Rgb3)
        : Array.from({ length: LOT_SLOT_COUNT }, () => [...DEFAULT_BORDER] as Rgb3),
      borderWidths: lot.borderWidths ? [...lot.borderWidths] : [0, 0, 0, 0],
      borderPatternIndices: lot.borderPatternIndices
        ? [...lot.borderPatternIndices]
        : [0, 0, 0, 0],
    },
    mask: data,
  };
}

/** 自绘地面存档 → Raster 编辑文档（重新进入绘制 sheet）。 */
export function schemaFromAuthored(lot: AuthoredLot): RasterSchema {
  const { meta } = lot;
  return {
    format: "openscp.raster",
    schema_version: 1,
    profile: "lot-mask",
    name: meta.name,
    source: null,
    image: { width: meta.width, height: meta.height, encoding: "rgba8-base64", data: lot.mask },
    parameters: null,
    sampling: { generateMips: false, metersPerPixel: meta.metersPerPixel },
    lot: {
      colors: meta.colors.map((color) => [...color] as Rgba4),
      baseTile: meta.baseTile,
      ...(meta.tilePeriod ? { tilePeriod: [...meta.tilePeriod] as [number, number] } : {}),
      size: [...meta.size] as [number, number],
      borderColors: meta.borderColors.map((color) => [...color] as Rgb3),
      borderPatternIndices: [...meta.borderPatternIndices],
      borderWidths: [...meta.borderWidths],
      slots: [...meta.slots],
      target: null,
    },
    decal: {
      colors: ["#ffffff", "#ffffff", "#ffffff", "#ffffff"],
      aspectRatio: 1,
      entryId: "",
      replaceInstance: "",
      atlas: null,
    },
  };
}

export function maskPngDataUrl(lot: AuthoredLot): string {
  const png = encodeRgbaPng(lot.meta.width, lot.meta.height, base64ToRgba(lot.mask));
  return `data:image/png;base64,${rgbaToBase64(new Uint8ClampedArray(png.buffer, png.byteOffset, png.byteLength))}`;
}

/**
 * 库卡片默认预览用的不透明 albedo。
 * 直接把 mask 当贴图会保留通道权重的低 alpha，再叠加地面材质
 * alphaTest 后整面被镂空，只剩边框——绘制器右侧预览走的是这条合成。
 */
export function authoredAlbedoDataUrl(lot: AuthoredLot): string {
  const pixels = base64ToRgba(lot.mask);
  const image = { data: pixels, width: lot.meta.width, height: lot.meta.height };
  const output = composeGroundPixels({
    mask: image,
    rawMask: image,
    baseTile: null,
    lotColors: lot.meta.colors,
    lotBorderColors: lot.meta.borderColors,
    lotBorderPatternIndices: lot.meta.borderPatternIndices,
    lotBorderWidths: lot.meta.borderWidths,
    normalAtlas: null,
    tilesX: 1,
    tilesY: 1,
  });
  const png = encodeRgbaPng(output.width, output.height, output.albedo);
  return `data:image/png;base64,${rgbaToBase64(new Uint8ClampedArray(png.buffer, png.byteOffset, png.byteLength))}`;
}

/** 绑定到工作台草稿地块时写入的 Lot 样式（贴图图集由调用方补齐）。 */
export function authoredLotStyle(lot: AuthoredLot) {
  const { meta } = lot;
  return {
    lotSize: [...meta.size] as [number, number],
    colors: meta.colors.map((color) => [...color] as Rgba4),
    colorsAuthored: meta.colors.map(() => true),
    borderColors: meta.borderColors.map((color) => [...color] as Rgb3),
    borderWidths: [...meta.borderWidths],
    borderPatternIndices: [...meta.borderPatternIndices],
    baseTile: meta.baseTile,
    tilePeriod: meta.tilePeriod ? ([...meta.tilePeriod] as [number, number]) : null,
    maskRawRgba: lot.mask,
    maskPng: maskPngDataUrl(lot),
  };
}
