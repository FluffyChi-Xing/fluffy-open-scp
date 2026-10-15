import { describe, it, expect } from "vitest";
import { stateResourcePixels, hasStateResource } from "./map-state-fields";
import { ownsRoadPoint } from "./map-native-roads";
import { createTerrainMaterial } from "./terrain-material";
import * as THREE from "three";
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
  it("keeps regional groundwater on ED and city groundwater inside its own domain", () => {
    const water = {
      ...data,
      heightDiv: 32,
      waterZ: -870,
      staticLayers: {
        sources: [
          {
            ...city,
            maps: [{ id: 0x0f92dc56, size: 2, values: [0, 65535, 32768, 0] }],
          },
        ],
      },
    } as Region3DData;
    expect(hasStateResource(water, "cityWaterTable")).toBe(true);
    expect(
      hasStateResource({ ...water, staticLayers: null }, "cityWaterTable"),
    ).toBe(false);
    const pixels = stateResourcePixels(water, "cityWaterTable", 4);
    expect(Array.from(pixels.slice(0, 4))).toEqual([0, 0, 0, 0]);
    expect(Array.from(pixels.slice(20, 24))).toEqual([0, 0, 0, 255]);
    expect(pixels[24]).toBe(255);
    const material = createTerrainMaterial(
      water,
      new Uint16Array(16),
      new Uint8ClampedArray(64).fill(255),
    );
    const shader = {
      uniforms: {},
      vertexShader: "",
      fragmentShader: "",
    } as Parameters<typeof material.onBeforeCompile>[0];
    material.onBeforeCompile(shader, {} as THREE.WebGLRenderer);
    material.setResource("waterTable", 0.75);
    expect(shader.uniforms.resourceChannel.value).toBe(2);
    expect(shader.uniforms.hasStaticResource.value).toBe(false);
    material.setResource("cityWaterTable", 0.75);
    expect(shader.uniforms.resourceChannel.value).toBe(3);
    expect(shader.uniforms.hasStaticResource.value).toBe(true);
    expect(Array.from(shader.uniforms.staticResource.value.image.data)).toEqual(
      Array.from(pixels),
    );
    material.setResource("waterTable", 0.75);
    expect(shader.uniforms.hasStaticResource.value).toBe(false);
    material.dispose();
  });
});
