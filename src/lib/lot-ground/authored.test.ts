import { describe, expect, it } from "vitest";
import { rgbaToBase64 } from "@/lib/raster-editor/encoders";
import {
  authoredAlbedoDataUrl,
  authoredFromSchema,
  authoredLotStyle,
  maskMatchesMeta,
  maskPngDataUrl,
  parseAuthoredLotMeta,
  schemaFromAuthored,
  type AuthoredLot,
} from "./authored";

const mask = rgbaToBase64(new Uint8ClampedArray([255, 0, 0, 0, 0, 0, 0, 0]));
const lot: AuthoredLot = {
  meta: {
    format: "openscp.lot-ground",
    version: 1,
    name: "Plaza",
    size: [1.5, 0.75],
    width: 2,
    height: 1,
    metersPerPixel: 0.75,
    baseTile: 8,
    tilePeriod: null,
    colors: [[60, 110, 40, 8], [128, 128, 128, 0], [128, 128, 128, 0], [128, 128, 128, 0]],
    slots: [true, false, false, false],
    borderColors: [[156, 156, 156], [156, 156, 156], [156, 156, 156], [156, 156, 156]],
    borderWidths: [0, 0, 0, 0],
    borderPatternIndices: [0, 0, 0, 0],
  },
  mask,
};

describe("authored lot ground", () => {
  it("round-trips through the raster editor schema", () => {
    expect(authoredFromSchema(schemaFromAuthored(lot))).toEqual(lot);
  });

  it("retains curb heights through project storage and workbench binding", () => {
    const schema = schemaFromAuthored(lot);
    schema.lot.colorHeights = [-2, -1, -2, -2];
    schema.lot.borderHeights = [-2.5, 0, -2, -2];
    const saved = authoredFromSchema(schema);
    const meta = parseAuthoredLotMeta(JSON.stringify(saved.meta))!;
    expect(schemaFromAuthored({ ...saved, meta }).lot.borderHeights).toEqual(schema.lot.borderHeights);
    expect(authoredLotStyle({ ...saved, meta }).colorHeights).toEqual(schema.lot.colorHeights);
    expect(authoredLotStyle(lot).colorHeights).toBeUndefined();
  });

  it("validates the stored metadata", () => {
    expect(parseAuthoredLotMeta(JSON.stringify(lot.meta))).toEqual(lot.meta);
    expect(parseAuthoredLotMeta("{}")).toBeNull();
    expect(parseAuthoredLotMeta("not json")).toBeNull();
    expect(maskMatchesMeta(lot.meta, mask)).toBe(true);
    expect(maskMatchesMeta({ ...lot.meta, width: 3 }, mask)).toBe(false);
  });

  it("derives the workbench lot style", () => {
    const style = authoredLotStyle(lot);
    expect(style.lotSize).toEqual([1.5, 0.75]);
    expect(style.maskRawRgba).toBe(mask);
    expect(style.maskPng.startsWith("data:image/png;base64,")).toBe(true);
    expect(style.colorsAuthored).toEqual([true, true, true, true]);
  });

  it("composes an opaque albedo preview distinct from the raw mask", () => {
    const albedo = authoredAlbedoDataUrl(lot);
    expect(albedo.startsWith("data:image/png;base64,")).toBe(true);
    expect(albedo).not.toBe(maskPngDataUrl(lot));
  });
});
