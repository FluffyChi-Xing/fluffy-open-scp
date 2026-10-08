import { describe, expect, it } from "vitest";
import { cellRandom, ecoChannel, fieldIndex, roadPreviewMask } from "./map-fields";

describe("region layer mapping", () => {
  it("uses the cropped world's origin and rejects positions outside its extent", () => {
    expect(fieldIndex(-8192, -8192, 1024, [-8192, -8192], 16)).toBe(0);
    expect(fieldIndex(-8176, -8160, 1024, [-8192, -8192], 16)).toBe(2049);
    expect(fieldIndex(8192, 0, 1024, [-8192, -8192], 16)).toBe(-1);
    expect(fieldIndex(-8200, 0, 1024, [-8192, -8192], 16)).toBe(-1);
  });
  it("maps only established ecology channels without inventing mineral deposits", () => {
    expect(["soil", "forest", "waterTable"].map(ecoChannel)).toEqual([0, 1, 2]);
    expect(["coal", "oil", "ore", null].map(ecoChannel)).toEqual([null, null, null, null]);
  });
  it("rejects a dry region and a groundwater-only zero strip as roads", () => {
    const pixels = new Uint8ClampedArray(16 * 16 * 4);
    expect(roadPreviewMask(pixels, 16).some(Boolean)).toBe(false);
    pixels.fill(255);
    for (let y = 0; y < 16; y++) pixels[(y * 16 + 8) * 4 + 2] = 0;
    expect(roadPreviewMask(pixels, 16).some(Boolean)).toBe(false);
    for (let y = 0; y < 16; y++) pixels[(y * 16 + 8) * 4 + 1] = 0;
    expect(roadPreviewMask(pixels, 16)[8 * 16 + 8]).toBe(1);
  });
  it("keeps tree placement deterministic and separates independent jitter channels", () => {
    expect(cellRandom(-10, 82, 1)).toBe(cellRandom(-10, 82, 1));
    expect(cellRandom(-10, 82, 1)).not.toBe(cellRandom(-10, 82, 2));
    expect(cellRandom(-10, 82, 1)).toBeGreaterThanOrEqual(0);
    expect(cellRandom(-10, 82, 1)).toBeLessThan(1);
  });
});
