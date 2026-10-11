import { describe, it, expect } from "vitest";
import { stateResourcePixels, hasStateResource } from "./map-state-fields";
import { ownsRoadPoint } from "./map-native-roads";
import type { Region3DData, StateSource } from "@/lib/region-map";

describe("static map registration", () => {
  const city: StateSource = {
    site: "1029",
    origin: [0, 0],
    bounds: [-16, -16, 16, 16],
    maps: [{ id: 0xd779c976, size: 2, values: [0, 65535, 32768, 0] }],
    curves: [],
  };
  const regional: StateSource = {
    site: "0",
    origin: [0, 0],
    bounds: null,
    maps: [],
    curves: [],
  };
  const data = {
    size: 4,
    metersPerPixel: 16,
    originWorld: [-32, -32],
    staticLayers: { sources: [regional, city] },
  } as Region3DData;
  it("distinguishes observed zero from unavailable cells and preserves asymmetric field orientation", () => {
    const p = stateResourcePixels(data, "coal", 4);
    expect(p[3]).toBe(0);
    expect(p[(1 * 4 + 1) * 4 + 3]).toBe(255);
    expect(p[(1 * 4 + 1) * 4]).toBe(0);
    expect(p[(1 * 4 + 2) * 4]).toBe(255);
    expect(p[(2 * 4 + 1) * 4]).toBe(128);
    expect(hasStateResource(data, "oil")).toBe(false);
  });
  it("keeps regional roads inside city bounds and excludes duplicate city roads", () => {
    expect(ownsRoadPoint(city)).toBe(false);
    expect(ownsRoadPoint(regional)).toBe(true);
  });
  it("clamps samples at the city edge and accepts case-insensitive water labels", () => {
    const edge = {
      ...data,
      staticLayers: {
        sources: [
          regional,
          {
            ...city,
            bounds: [-16, -16, 16, 16],
            maps: [{ id: 0x0f92dc56, size: 2, values: [65535, 0, 0, 65535] }],
          },
        ],
      },
    } as Region3DData;
    expect(hasStateResource(edge, "watertable")).toBe(true);
    expect(() => stateResourcePixels(edge, "watertable", 4)).not.toThrow();
  });
});
