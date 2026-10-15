import { describe, expect, it } from "vitest";
import { applyBuildingSlots, type ProjectAsset } from "./assetFlow";
import type { LotModelPayload } from "@/api/tauri";
const png = (tag: string): ProjectAsset => ({
  asset: tag + ".png",
  path: "",
  size: 8,
  base64: btoa(String.fromCharCode(137, 80, 78, 71, 13, 10, 26, 10)),
});
const source = {
  materials: [{ paramCols: 2 }],
  glbs: [],
  meshMaterialIndices: [],
  meshUvKinds: [],
  diagnostics: "",
} as unknown as LotModelPayload;
describe("building input slots", () => {
  it("maps five image inputs independently and preserves source payload", () => {
    const result = applyBuildingSlots(source, [
      null,
      png("color"),
      png("normal"),
      png("shader"),
      png("palette"),
      png("room"),
    ]);
    for (const key of [
      "tintPng",
      "normalPng",
      "shaderPng",
      "palettePng",
      "interiorPng",
    ] as const)
      expect(result.materials[0][key]?.length).toBe(8);
    expect(source.materials[0].normalPng).toBeUndefined();
  });
  it("rejects parameter tables that cannot address the mesh columns", () => {
    const params = {
      asset: "params.json",
      path: "",
      size: 0,
      base64: btoa(JSON.stringify({ cols: 1, values: Array(16).fill(0) })),
    };
    expect(() => applyBuildingSlots(source, [params])).toThrow(/列数/);
    params.base64 = btoa(
      JSON.stringify({ cols: 2, values: Array(32).fill(1) }),
    );
    expect(
      applyBuildingSlots(source, [params]).materials[0].paramsF32?.length,
    ).toBe(32);
  });
  it("does not reinterpret unrelated shader families as building4", () => {
    const ordinary = {
      ...source,
      materials: [{ ...source.materials[0], paramCols: 0 }],
    };
    expect(() => applyBuildingSlots(ordinary, [null, png("color")])).toThrow(
      /building4/,
    );
  });
  it("rejects invalid images and missing material targets", () => {
    expect(() =>
      applyBuildingSlots(source, [null, { ...png("bad"), base64: "" }]),
    ).toThrow(/PNG/);
    expect(() => applyBuildingSlots(source, [], 1)).toThrow(/材质/);
  });
});
