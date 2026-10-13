import { i18n } from "@/locales";
const t = i18n.global.t;
import type { Tgi } from "@/api/tauri";
import { base64ToRgba } from "./encoders";

export const RASTER_PROFILES = [
  {
    id: "slot0",
    get label() {
      return t("rasterEditor.text127");
    },
    get group() {
      return t("rasterEditor.text080");
    },
    get hint() {
      return t("rasterEditor.text128");
    },
  },
  {
    id: "slot1",
    get label() {
      return t("rasterEditor.text129");
    },
    get group() {
      return t("rasterEditor.text080");
    },
    get hint() {
      return t("rasterEditor.text130");
    },
  },
  {
    id: "slot2",
    get label() {
      return t("rasterEditor.text131");
    },
    get group() {
      return t("rasterEditor.text080");
    },
    get hint() {
      return t("rasterEditor.text132");
    },
  },
  {
    id: "slot3",
    label: "Shader Map",
    get group() {
      return t("rasterEditor.text080");
    },
    get hint() {
      return t("rasterEditor.text133");
    },
  },
  {
    id: "slot4",
    get label() {
      return t("rasterEditor.text134");
    },
    get group() {
      return t("rasterEditor.text080");
    },
    get hint() {
      return t("rasterEditor.text135");
    },
  },
  {
    id: "slot5",
    get label() {
      return t("rasterEditor.text136");
    },
    get group() {
      return t("rasterEditor.text080");
    },
    get hint() {
      return t("rasterEditor.text137");
    },
  },
  {
    id: "lot-mask",
    get label() {
      return t("rasterEditor.text138");
    },
    get group() {
      return t("rasterEditor.text081");
    },
    get hint() {
      return t("rasterEditor.text139");
    },
  },
  {
    id: "lot-texture",
    get label() {
      return t("rasterEditor.text140");
    },
    get group() {
      return t("rasterEditor.text081");
    },
    get hint() {
      return t("rasterEditor.text141");
    },
  },
  {
    id: "decal-sign",
    get label() {
      return t("rasterEditor.text142");
    },
    group: "Decal",
    get hint() {
      return t("rasterEditor.text143");
    },
  },
  {
    id: "decal-graffiti",
    get label() {
      return t("rasterEditor.text144");
    },
    group: "Decal",
    get hint() {
      return t("rasterEditor.text145");
    },
  },
  {
    id: "decal-sdf",
    get label() {
      return t("rasterEditor.text146");
    },
    group: "Decal",
    get hint() {
      return t("rasterEditor.text147");
    },
  },
  {
    id: "decal-interior",
    get label() {
      return t("rasterEditor.text148");
    },
    group: "Decal",
    get hint() {
      return t("rasterEditor.text149");
    },
  },
  {
    id: "decal-rgba",
    get label() {
      return t("rasterEditor.text150");
    },
    group: "Decal",
    get hint() {
      return t("rasterEditor.text151");
    },
  },
  {
    id: "rgba",
    get label() {
      return t("rasterEditor.text152");
    },
    get group() {
      return t("rasterEditor.text082");
    },
    get hint() {
      return t("rasterEditor.text153");
    },
  },
] as const;
export type RasterProfile = (typeof RASTER_PROFILES)[number]["id"];
export type RasterTool =
  | "brush"
  | "eraser"
  | "line"
  | "rect"
  | "fill"
  | "picker"
  | "parking"
  | "curve"
  | "rect-fill"
  | "polyline"
  | "polygon"
  | "layer-picker";
export type Channel = "rgba" | "r" | "g" | "b" | "a";
export interface ParameterTable {
  cols: number;
  values: number[];
}
export interface RasterSource {
  packageId?: number;
  name: string;
  tgi?: Tgi;
}
export interface RasterSchema {
  format: "openscp.raster";
  schema_version: 1;
  profile: RasterProfile;
  name: string;
  source: RasterSource | null;
  image: {
    width: number;
    height: number;
    encoding: "rgba8-base64";
    data: string;
  } | null;
  parameters: ParameterTable | null;
  sampling: { generateMips: boolean; metersPerPixel: number };
  lot: {
    colors: [number, number, number, number][];
    target: RasterSource | null;
  };
  decal: {
    colors: string[];
    aspectRatio: number;
    entryId: string;
    entryTgi?: Tgi;
    replaceInstance: string;
    atlas: RasterSource | null;
  };
}
export function isMaskProfile(profile: RasterProfile) {
  return profile === "lot-mask" || profile === "decal-sign" || profile === "decal-graffiti";
}
export function validateParameters(value: unknown): ParameterTable {
  const p = value as ParameterTable | null;
  if (
    !p ||
    !Number.isInteger(p.cols) ||
    p.cols < 1 ||
    p.cols > 4096 ||
    !Array.isArray(p.values) ||
    p.values.length !== p.cols * 16 ||
    !p.values.every(
      (v) =>
        typeof v === "number" &&
        Number.isFinite(v) &&
        Number.isFinite(Math.fround(v)),
    )
  )
    throw new Error(t("rasterEditor.text154"));
  return { cols: p.cols, values: [...p.values] };
}
export function validateSize(width: number, height: number) {
  if (
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > 4096 ||
    height > 4096
  )
    throw new Error(t("rasterEditor.text155"));
}
export function parseRasterSchema(input: unknown): RasterSchema {
  const s = typeof input === "string" ? JSON.parse(input) : input;
  if (
    !s ||
    s.format !== "openscp.raster" ||
    s.schema_version !== 1 ||
    !RASTER_PROFILES.some((p) => p.id === s.profile) ||
    typeof s.name !== "string"
  )
    throw new Error(t("rasterEditor.text156"));
  if (s.profile === "slot0") validateParameters(s.parameters);
  else {
    if (
      !s.image ||
      s.image.encoding !== "rgba8-base64" ||
      typeof s.image.data !== "string"
    )
      throw new Error(t("rasterEditor.text157"));
    validateSize(s.image.width, s.image.height);
    if (
      s.image.data.length > 90_000_000 ||
      base64ToRgba(s.image.data).length !== s.image.width * s.image.height * 4
    )
      throw new Error(t("rasterEditor.text158"));
  }
  if (
    !s.sampling ||
    typeof s.sampling.generateMips !== "boolean" ||
    !Number.isFinite(s.sampling.metersPerPixel) ||
    s.sampling.metersPerPixel <= 0
  )
    throw new Error(t("rasterEditor.text159"));
  if (
    !Array.isArray(s.lot?.colors) ||
    s.lot.colors.length !== 4 ||
    !s.lot.colors.every(
      (c: number[]) =>
        Array.isArray(c) &&
        c.length === 4 &&
        c.every(
          (n, i) => Number.isInteger(n) && n >= 0 && n <= (i === 3 ? 15 : 255),
        ),
    )
  )
    throw new Error(t("rasterEditor.text160"));
  if (
    !Array.isArray(s.decal?.colors) ||
    s.decal.colors.length !== 4 ||
    !s.decal.colors.every((c: string) => /^#[\da-f]{6}$/i.test(c)) ||
    !Number.isFinite(s.decal.aspectRatio) ||
    s.decal.aspectRatio <= 0 ||
    typeof s.decal.entryId !== "string" ||
    typeof s.decal.replaceInstance !== "string"
  )
    throw new Error(t("rasterEditor.text161"));
  for (const source of [
    s.source,
    s.lot.target,
    s.decal.atlas,
    s.decal.entryTgi ? { name: "entry", tgi: s.decal.entryTgi } : null,
  ]) {
    if (
      source != null &&
      (typeof source.name !== "string" ||
        (source.tgi &&
          !["typeId", "group", "instance"].every(
            (k) =>
              Number.isInteger(source.tgi[k]) &&
              source.tgi[k] >= 0 &&
              source.tgi[k] <= 0xffffffff,
          )))
    )
      throw new Error(t("rasterEditor.text162"));
  }
  // Hosts may pass reactive Vue props; structuredClone rejects proxies.
  return JSON.parse(JSON.stringify(s)) as RasterSchema;
}
export function channelPixels(pixels: Uint8ClampedArray, channel: Channel) {
  if (channel === "rgba") return pixels;
  const index = { r: 0, g: 1, b: 2, a: 3 }[channel];
  const out = new Uint8ClampedArray(pixels.length);
  for (let i = 0; i < pixels.length; i += 4) {
    out[i] = out[i + 1] = out[i + 2] = pixels[i + index];
    out[i + 3] = 255;
  }
  return out;
}
export function restoreOtherChannels(
  pixels: Uint8ClampedArray,
  before: Uint8ClampedArray,
  channel: Channel,
) {
  if (channel === "rgba") return;
  const index = { r: 0, g: 1, b: 2, a: 3 }[channel];
  for (let i = 0; i < pixels.length; i++)
    if (i % 4 !== index) pixels[i] = before[i];
}
