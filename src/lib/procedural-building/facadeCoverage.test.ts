import { describe, expect, it } from "vitest";
import type { LotMaterialTextures } from "@/api/tauri";
import type { SurfaceMaterial } from "./propertyMaterial";
import {
  measureFacadeCoverage,
  type BuildingAlphaMaps,
} from "./facadeCoverage";

function fixture() {
  const material = {
    paramCols: 1,
    paramsF32: new Float32Array([
      0,
      0,
      0,
      0,
      0.5,
      1,
      0,
      0, // opaque Base region (left pixel)
      0.5,
      1,
      0.5,
      0, // window Top region (right pixel)
      0,
      0,
      1,
      1,
    ]),
  } as LotMaterialTextures;
  const surface: SurfaceMaterial = {
    materialIndex: 0,
    column: 0,
    seed: 0.25,
    windowCoverage: 0,
    topCoverage: 0,
    base: { offset: [0, 0], horizontal: [1, 0], vertical: [0, 1] },
    top: { offset: [0, 0], horizontal: [1, 0], vertical: [0, 1] },
  };
  const maps: BuildingAlphaMaps = {
    tint: { width: 2, height: 1, data: new Uint8Array([255, 255]) },
    shader: { width: 2, height: 1, data: new Uint8Array([255, 0]) },
  };
  const measure = () => measureFacadeCoverage(material, surface, maps);
  return { material, surface, maps, measure };
}

describe("transferred building4 window coverage", () => {
  it("honours Top padding instead of judging the whole atlas motif", () => {
    const { material, measure } = fixture();
    expect(measure().windowCoverage).toBe(1);
    material.paramsF32!.set([1, 1], 12);
    expect(measure().windowCoverage).toBeCloseTo(0.25, 1);
    material.paramsF32!.set([80000, 80000], 12);
    expect(measure().windowCoverage).toBe(0);
  });

  it("retains constant UV knife bands despite large padding", () => {
    const { material, surface, measure } = fixture();
    material.paramsF32![13] = 80000;
    surface.top.offset[1] = 0.5;
    surface.top.vertical[1] = 0;
    expect(measure().windowCoverage).toBe(1);
    surface.top.offset[1] = 0.4;
    expect(measure().windowCoverage).toBe(0);
  });

  it("accepts mirrored regions and rejects disabled Top regions", () => {
    const { material, measure } = fixture();
    material.paramsF32!.set([-0.5, -1, 1, 1], 8);
    expect(measure().windowCoverage).toBe(1);
    material.paramsF32![8] = 0;
    expect(measure().windowCoverage).toBe(0);
  });

  it("uses tint alpha as the Top blend and clips transparent Base fragments", () => {
    const { maps, measure } = fixture();
    maps.tint.data[1] = 0;
    expect(measure().windowCoverage).toBe(0);
    maps.tint.data[1] = 128;
    expect(measure().windowCoverage).toBeCloseTo(128 / 255);
    maps.tint.data[0] = 0;
    expect(measure().windowCoverage).toBe(0);
  });

  it("keeps Base glass visible when Top is disabled", () => {
    const { material, measure } = fixture();
    material.paramsF32!.set([0.5, 1, 0.5, 0], 4);
    material.paramsF32![8] = 0;
    expect(measure()).toEqual({ windowCoverage: 1, topCoverage: 0 });
  });
});
