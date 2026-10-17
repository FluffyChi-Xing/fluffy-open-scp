import { describe, it, expect } from "vitest";
import {
  generateHeightField,
  buildVegetationMasks,
  type MapGenerationConfig,
} from "./map-generation";
const config: MapGenerationConfig = {
  width: 32,
  height: 24,
  seed: 42,
  algorithm: "perlin",
  octaves: 4,
  frequency: 4,
  persistence: 0.5,
  grassBrush: 1,
  treeBrush: 0,
};
describe("map generation demo", () => {
  it("produces repeatable, nonconstant rectangular terrain and changes with the seed", () => {
    const a = generateHeightField(config);
    expect(a).toHaveLength(32 * 24);
    expect(a).toEqual(generateHeightField(config));
    expect(a).not.toEqual(generateHeightField({ ...config, seed: 43 }));
    expect(Math.min(...a)).toBe(0);
    expect(Math.max(...a)).toBe(1);
  });
  it("uses distinct gradient and value algorithms", () => {
    expect(generateHeightField(config)).not.toEqual(
      generateHeightField({ ...config, algorithm: "value" }),
    );
  });
  it("respects vegetation strength and reduces grass on steep slopes", () => {
    const flat = new Float32Array(32 * 24).fill(0.5);
    const masks = buildVegetationMasks(flat, 32, 24, config);
    expect(masks.grass.every((v) => v === 255)).toBe(true);
    expect(masks.trees.every((v) => v === 0)).toBe(true);
    flat[12] = 1;
    expect(buildVegetationMasks(flat, 32, 24, config).grass[11]).toBeLessThan(
      255,
    );
  });
});
