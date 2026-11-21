import { describe, it, expect } from "vitest";
import { roadPreviewGroundHeight } from "./roadPreviewGround";
import type { Region3DData } from "@/lib/region-map";
type Assets = NonNullable<Region3DData["roadAssets"]>;
const sweep = (
  model: number,
  relativeToGround = false,
  stackToGround = false,
) => ({
  model,
  relativeToGround,
  stackToGround,
  offset: [0, 0, 0] as [number, number, number],
  scale: [1, 1, 1] as [number, number, number],
  rotation: [0, 0, 0] as [number, number, number],
  instance: true,
  interval: 32,
  start: 0,
  end: 0,
  length: 0,
  step: 0,
  distort: false,
  repeatUv: false,
  roundIntervals: false,
});
function fixture(): Assets {
  return {
    textures: {},
    instances: [],
    unsupportedComponents: [],
    ribbons: {},
    sweeps: { 1: [sweep(2), sweep(3, true), sweep(4, false, true)] },
    models: Object.fromEntries(
      [
        [2, [-1, -1, -3, 1, 1, 0]],
        [3, [-1, -1, -10, 1, 1, 7]],
        [4, [-1, -1, -20, 1, 1, 0]],
      ].map(([id, positions]) => [
        id,
        {
          id,
          positions,
          normals: [],
          uvs: [],
          indices: [],
          diffusePngBase64: "",
        },
      ]),
    ),
  } as Assets;
}
describe("bridge inspector terrain context", () => {
  it("keeps terrain-relative footings below the fixed bridge structure without editing asset vertices", () => {
    const a = fixture();
    const before = JSON.stringify(a.models);
    expect(roadPreviewGroundHeight(a, 1)).toBe(-10);
    expect(JSON.stringify(a.models)).toBe(before);
  });
  it("does not move the ground for ordinary paths or ground-relative street props", () => {
    const a = fixture();
    a.sweeps[1] = a.sweeps[1].filter((s) => !s.stackToGround);
    expect(roadPreviewGroundHeight(a, 1)).toBe(0);
  });
  it("uses native uniform instance scale and rotated model bounds", () => {
    const a = fixture();
    a.sweeps[1][1].scale = [2, 1, 100];
    expect(roadPreviewGroundHeight(a, 1)).toBe(-17);
    a.sweeps[1][1].scale = [1, 1, 1];
    a.sweeps[1][1].rotation = [180, 0, 0];
    expect(roadPreviewGroundHeight(a, 1)).toBeCloseTo(-13);
  });
});
