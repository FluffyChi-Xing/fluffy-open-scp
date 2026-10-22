import { describe, expect, it } from "vitest";
import { generateMassParts, massPreset, normalizeBlockMass } from "./massing";
import { buildMassFaces, type SolidFace } from "./solidGeometry";
import { buildSlotGeometries } from "./meshBuilder";
import type { BlockMassParameters } from "./schema";

function volume(faces: SolidFace[]) {
  let v = 0;
  for (const { vertices } of faces)
    for (let i = 1; i + 1 < vertices.length; i++)
      v += vertices[0].dot(vertices[i].clone().cross(vertices[i + 1])) / 6;
  return v;
}
const cube = (): BlockMassParameters => ({
  blocks: [{ id: "a", center: [0, 0, 5], size: [10, 10, 10], rotation: 0 }],
  voids: [],
});
const facesFor = (m: BlockMassParameters) =>
  buildMassFaces(generateMassParts(m));

describe("metric massing solids", () => {
  it("keeps dimensions over 100m and heights over 300m with six flat faces", () => {
    const m = cube();
    m.blocks[0].size = [180, 140, 360];
    m.blocks[0].center[2] = 180;
    const f = facesFor(m);
    expect(f).toHaveLength(6);
    expect(volume(f)).toBeCloseTo(180 * 140 * 360);
    expect(
      buildSlotGeometries(generateMassParts(m)).get("roof")!.getIndex()!.count,
    ).toBe(12);
  });
  it("unions overlapping and touching blocks without internal or duplicate roof faces", () => {
    for (const x of [0, 5, 10, 15]) {
      const m = cube();
      m.blocks.push({ ...m.blocks[0], id: "b", center: [x, 0, 5] });
      const f = facesFor(m);
      expect(volume(f)).toBeCloseTo(1000 + Math.min(x, 10) * 100);
      const roofArea = f
        .filter((p) => p.normal.z > 0.9)
        .reduce((a, p) => {
          for (let i = 1; i + 1 < p.vertices.length; i++)
            a +=
              p.vertices[i]
                .clone()
                .sub(p.vertices[0])
                .cross(p.vertices[i + 1].clone().sub(p.vertices[0]))
                .length() / 2;
          return a;
        }, 0);
      expect(roofArea).toBeCloseTo(100 + Math.min(x, 10) * 10);
    }
  });
  it("cuts a recess with a back wall and a courtyard through the roof and base", () => {
    const m = cube();
    m.voids.push({
      id: "h",
      blockId: "a",
      center: [0, -5, 0],
      size: [4, 4, 6],
    });
    const recessed = facesFor(m);
    expect(volume(recessed)).toBeCloseTo(1000 - 4 * 2 * 6);
    expect(
      recessed.some(
        (p) =>
          p.normal.y < -0.9 &&
          p.vertices.every((v) => Math.abs(v.y + 3) < 1e-5),
      ),
    ).toBe(true);
    m.voids[0] = { id: "h", blockId: "a", center: [0, 0, 0], size: [4, 4, 12] };
    expect(volume(facesFor(m))).toBeCloseTo(840);
  });
  it("rotates cavities with their host and subtracts only from that host", () => {
    const m = cube();
    m.blocks[0].rotation = 37;
    m.voids.push({
      id: "h",
      blockId: "a",
      center: [0, 0, 0],
      size: [4, 4, 12],
    });
    expect(volume(facesFor(m))).toBeCloseTo(840);
    m.blocks.push({
      id: "b",
      center: [0, 0, 5],
      size: [4, 4, 10],
      rotation: 37,
    });
    expect(volume(facesFor(m))).toBeCloseTo(1000);
  });
  it("handles an entirely removed solid and normalizes nonfinite data", () => {
    const m = cube();
    m.voids.push({
      id: "h",
      blockId: "a",
      center: [0, 0, 0],
      size: [20, 20, 20],
    });
    expect(facesFor(m)).toHaveLength(0);
    m.blocks[0].size = [NaN, Infinity, -10];
    m.blocks[0].center = [NaN, -Infinity, -100];
    expect(normalizeBlockMass(m).blocks[0]).toMatchObject({
      size: [48, 24, 1],
      center: [0, 0, 0.5],
    });
  });
  it("builds a Y and courtyard with finite outward faces", () => {
    for (const name of ["y", "courtyard"] as const) {
      const f = facesFor(massPreset(name));
      expect(volume(f)).toBeGreaterThan(0);
      expect(f.length).toBeLessThan(250);
      for (const p of f) expect(p.normal.length()).toBeCloseTo(1);
    }
  });
});
