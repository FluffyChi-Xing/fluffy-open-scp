import { describe, expect, it } from "vitest";
import { Mesh, Vector3 } from "three";
import { spacedFacadeFaces } from "./windowSpacing";
import type {
  BuildingMaterialSource,
  SurfaceMaterial,
} from "./propertyMaterial";
import { buildMassFaces, type SolidFace } from "./solidGeometry";
import { buildModelPayload } from "./meshBuilder";
import { parseLotModelObjects } from "@/lib/three-gltf";

const surface = (column: number): SurfaceMaterial => ({
  materialIndex: 0,
  column,
  seed: 0.25,
  base: { offset: [0, 0], horizontal: [0.25, 0], vertical: [0, 0.25] },
  top: { offset: [0, 0], horizontal: [0.5, 0], vertical: [0, 0.25] },
  topCoverage: 1,
  windowCoverage: column === 1 ? 0.5 : 0,
});
const source: BuildingMaterialSource = {
  facade: surface(1),
  roof: surface(2),
  wall: surface(3),
  payload: {
    glbs: [],
    materials: [],
    meshMaterialIndices: [],
    meshUvKinds: [],
    diagnostics: "",
  },
};
const front: SolidFace = {
  normal: new Vector3(0, -1, 0),
  vertices: [
    [-6, 0, 0],
    [6, 0, 0],
    [6, 0, 10],
    [-6, 0, 10],
  ].map((p) => new Vector3(...p)),
};
const area = (f: SolidFace) =>
  f.vertices.slice(1, -1).reduce(
    (sum, p, i) =>
      sum +
      p
        .clone()
        .sub(f.vertices[0])
        .cross(f.vertices[i + 2].clone().sub(f.vertices[0]))
        .length() /
        2,
    0,
  );

describe("window spacing in metres", () => {
  it("inserts one metre wall strips without stretching windows, including negative cycles", () => {
    const pieces = spacedFacadeFaces(front, source, 1);
    expect(pieces.reduce((s, p) => s + area(p.face), 0)).toBeCloseTo(120);
    const widths = (column: number) =>
      pieces
        .filter((p) => p.surface.column === column)
        .map(
          (p) =>
            Math.max(...p.face.vertices.map((v) => v.x)) -
            Math.min(...p.face.vertices.map((v) => v.x)),
        );
    expect(widths(1)).toEqual([2, 2, 2, 2]);
    expect(widths(3)).toEqual([1, 1, 1, 1]);
    for (const { face, surface: s } of pieces.filter(
      (p) => p.surface.column === 1,
    )) {
      expect(s.top.horizontal).toEqual(source.facade.top.horizontal);
      expect(s.base.horizontal).toEqual(source.facade.base.horizontal);
      const min = Math.min(...face.vertices.map((v) => v.x));
      expect((s.top.offset[0] + min * 0.5) % 1).toBeCloseTo(0);
    }
  });
  it("uses Base repeats for glass and roof fallback for blank strips", () => {
    const s = {
      ...source,
      facade: { ...source.facade, topCoverage: 0 },
      wall: undefined,
    };
    const pieces = spacedFacadeFaces(front, s, 1);
    const fullWindow = pieces.find(
      (p) => p.surface.column === 1 && p.face.vertices.some((v) => v.x === 0),
    )!;
    expect(
      Math.max(...fullWindow.face.vertices.map((v) => v.x)) -
        Math.min(...fullWindow.face.vertices.map((v) => v.x)),
    ).toBe(4);
    expect(pieces.some((p) => p.surface === s.roof)).toBe(true);
  });
  it("preserves area and normals on rotated cavity faces", () => {
    const faces = buildMassFaces([
      {
        id: "mass",
        kind: "mass",
        slot: "wall",
        center: [0, 0, 5],
        size: [10, 10, 10],
        rotation: 0.7,
        cutouts: [{ center: [0, -5, 0], size: [4, 4, 6] }],
      },
    ]);
    expect(faces.some((f) => f.normal.z < -0.9 && f.vertices[0].z > 1)).toBe(
      true,
    );
    for (const face of faces) {
      const pieces = spacedFacadeFaces(face, source, 1.5);
      expect(pieces.reduce((s, p) => s + area(p.face), 0)).toBeCloseTo(
        area(face),
        5,
      );
      for (const p of pieces) expect(p.face.normal).toEqual(face.normal);
      if (Math.abs(face.normal.z) > 0.7)
        expect(pieces).toEqual([{ face, surface: source.roof }]);
    }
  });
  it("keeps zero and nonfinite spacing unchanged", () => {
    for (const n of [0, NaN, Infinity, -1])
      expect(spacedFacadeFaces(front, source, n)).toEqual([
        { face: front, surface: source.facade },
      ]);
  });
  it("exports roof selectors on bottom and cavity ceiling in GLB", async () => {
    const payload = await buildModelPayload({
      parts: [
        {
          id: "m",
          kind: "mass",
          slot: "wall",
          center: [0, 0, 5],
          size: [10, 10, 10],
          cutouts: [{ center: [0, -5, 0], size: [4, 4, 6] }],
        },
      ],
      buildingMaterial: source,
      windowSpacing: 1,
    });
    const roots = await parseLotModelObjects(payload.glbs);
    let ceilings = 0,
      bottoms = 0;
    roots.forEach((root) =>
      root.traverse((o) => {
        if (!(o instanceof Mesh)) return;
        const pos = o.geometry.getAttribute("position"),
          n = o.geometry.getAttribute("normal"),
          uv = o.geometry.getAttribute("uv1");
        for (let i = 0; i < pos.count; i++)
          if (n.getZ(i) < -0.9) {
            expect(uv.getX(i) * 255).toBeCloseTo(2);
            if (pos.getZ(i) > 1) ceilings++;
            else bottoms++;
          }
        o.geometry.dispose();
      }),
    );
    expect(ceilings).toBeGreaterThan(0);
    expect(bottoms).toBeGreaterThan(0);
  });
});
