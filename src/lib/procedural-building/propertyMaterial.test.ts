import { describe, expect, it, vi } from "vitest";
import * as THREE from "three";
import { GLTFExporter } from "three/examples/jsm/exporters/GLTFExporter.js";
import type { LotMaterialTextures, LotModelPayload } from "@/api/tauri";
import { parseLotModelObjects } from "@/lib/three-gltf";
import { analyzeBuildingMaterial, parsePropertyTgi } from "./propertyMaterial";
import { buildModelPayload } from "./meshBuilder";
import { readBuildingAlphaMaps } from "./facadeCoverage";

vi.mock("./facadeCoverage", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./facadeCoverage")>()),
  // Decode only is mocked; coverage and material selection run in production code.
  readBuildingAlphaMaps: vi.fn(async () => ({
    tint: { width: 1, height: 1, data: new Uint8Array([255]) },
    shader: { width: 1, height: 1, data: new Uint8Array([255]) },
  })),
}));

export function fixtureMaterial(): LotMaterialTextures {
  const png = new Uint8Array([137, 80, 78, 71]);
  return {
    baseColorPng: null,
    slot0Png: null,
    normalPng: png,
    roughnessPng: png,
    aoPng: png,
    tintPng: png,
    palettePng: png,
    shaderPng: png,
    interiorPng: png,
    reliefPng: null,
    paramsF32: new Float32Array(16 * 32),
    paramCols: 32,
  };
}

async function sourceTriangle(
  size: number,
  column: number,
  roof = false,
  height = 12,
  repeatingTop = false,
) {
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute(
    "position",
    new THREE.Float32BufferAttribute(
      roof
        ? [0, 0, height, size, 0, height, 0, size, height]
        : [0, 0, 0, size, 0, 0, 0, 0, size],
      3,
    ),
  );
  geometry.setAttribute(
    "uv1",
    new THREE.Float32BufferAttribute(
      [
        column / 255,
        0.25,
        0,
        0,
        column / 255,
        0.25,
        0,
        0,
        column / 255,
        0.25,
        0,
        0,
      ],
      4,
    ),
  );
  geometry.setAttribute(
    "uv2",
    new THREE.Float32BufferAttribute([0, 0, size / 2, 0, 0, size / 4], 2),
  );
  // Constant Top V is intentional: do not invent window repetition here.
  geometry.setAttribute(
    "uv3",
    new THREE.Float32BufferAttribute(
      [0, 0.5, size / 3, 0.5, 0, repeatingTop ? 0.5 + size / 3 : 0.5],
      2,
    ),
  );
  geometry.computeVertexNormals();
  const material = new THREE.MeshStandardMaterial();
  const result = await new GLTFExporter().parseAsync(
    new THREE.Mesh(geometry, material),
    { binary: true },
  );
  geometry.dispose();
  material.dispose();
  return result as ArrayBuffer;
}

describe("Property material mapping", () => {
  it("requires a complete Property TGI and accepts whitespace/case", () => {
    expect(parsePropertyTgi(" 00B1B104:61F0C000:1D261FDD ")).toEqual({
      typeId: 0x00b1b104,
      group: 0x61f0c000,
      instance: 0x1d261fdd,
    });
    for (const value of [
      "1d261fdd",
      "2f4e681b:61f0c000:1d261fdd",
      "00b1b104:61f0c000:1d261fddjunk",
    ])
      expect(parsePropertyTgi(value)).toBeNull();
  });

  it("selects area-dominant real bindings and preserves UVs through generated GLBs", async () => {
    const payload: LotModelPayload = {
      glbs: [
        await sourceTriangle(1, 0),
        await sourceTriangle(10, 7),
        await sourceTriangle(8, 18, true),
        await sourceTriangle(30, 1, true, 0),
      ],
      materials: [fixtureMaterial(), fixtureMaterial()],
      meshMaterialIndices: [0, 1, 0, 0],
      meshUvKinds: [2, 2, 2, 2],
      diagnostics: "",
    };
    const source = await analyzeBuildingMaterial(payload);
    expect(source.facade.column).toBe(7);
    expect(source.facade.materialIndex).toBe(1);
    expect(source.roof.column).toBe(18);
    expect(source.facade.base.horizontal[0]).toBeCloseTo(0.5);
    expect(source.facade.base.vertical[1]).toBeCloseTo(0.25);
    expect(source.facade.top.vertical[1]).toBe(0);
    const generated = await buildModelPayload({
      parts: [
        {
          id: "wall",
          kind: "wall",
          slot: "wall",
          center: [0, 0, 4],
          size: [6, 6, 8],
        },
        {
          id: "ground",
          kind: "slab",
          slot: "ground",
          center: [0, 0, 0],
          size: [12, 12, 0.1],
        },
      ],
      buildingMaterial: source,
    });
    expect(generated.materials[1]).toBe(payload.materials[1]);
    expect(generated.meshMaterialIndices).toEqual([1, 0, 2]);
    expect(generated.meshUvKinds).toEqual([2, 2, 1]);
    const roots = await parseLotModelObjects(generated.glbs);
    roots[0].traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return;
      const geometry = object.geometry;
      const selector = geometry.getAttribute("uv1");
      expect(selector.itemSize).toBe(4);
      expect(selector.getX(0) * 255).toBeCloseTo(7);
      const normal = geometry.getAttribute("normal");
      const position = geometry.getAttribute("position");
      for (let i = 0; i < position.count; i++) {
        expect(normal.getZ(i)).toBe(0);
        expect(geometry.getAttribute("uv2").getY(i)).toBeCloseTo(
          position.getZ(i) / 4,
        );
        expect(geometry.getAttribute("uv3").getY(i)).toBeCloseTo(0.5);
      }
    });
  });

  it("rejects ordinary diffuse meshes rather than marking them building4", async () => {
    await expect(
      analyzeBuildingMaterial({
        glbs: [await sourceTriangle(10, 7)],
        materials: [fixtureMaterial()],
        meshMaterialIndices: [0],
        meshUvKinds: [1],
        diagnostics: "",
      }),
    ).rejects.toThrow("No usable building4");
  });

  it("chooses a substantial window facade over the largest brick wall and tiny glass decoration", async () => {
    const material = fixtureMaterial();
    const set = (row: number, column: number, values: number[]) =>
      material.paramsF32!.set(values, (row * material.paramCols + column) * 4);
    // Left half = opaque brick; right half = window. Same atlas, different columns.
    for (const col of [0, 5, 23]) set(1, col, [0.5, 1, 0, 0]);
    set(2, 5, [0.5, 1, 0.5, 0]);
    set(3, 5, [1, 1, 1, 1]);
    set(1, 23, [0.5, 1, 0.5, 0]);
    vi.mocked(readBuildingAlphaMaps).mockResolvedValueOnce({
      tint: { width: 2, height: 1, data: new Uint8Array([255, 255]) },
      shader: { width: 2, height: 1, data: new Uint8Array([255, 0]) },
    });
    const source = await analyzeBuildingMaterial({
      glbs: [
        await sourceTriangle(20, 0),
        await sourceTriangle(12, 5, false, 12, true),
        await sourceTriangle(1, 23),
      ],
      materials: [material],
      meshMaterialIndices: [0, 0, 0],
      meshUvKinds: [2, 2, 2],
      diagnostics: "",
    });
    expect(source.facade.column).toBe(5);
    expect(source.facade.windowCoverage).toBeGreaterThan(0.2);
    expect(source.facade.windowCoverage).toBeLessThan(0.3);
    expect(readBuildingAlphaMaps).toHaveBeenLastCalledWith(material);
  });

  it("keeps an area-dominant Base glass facade instead of a small Top window panel", async () => {
    const material = fixtureMaterial();
    material.paramsF32!.set([0.5, 1, 0.5, 0], (material.paramCols + 7) * 4);
    material.paramsF32!.set([0.5, 1, 0, 0], (material.paramCols + 5) * 4);
    material.paramsF32!.set([0.5, 1, 0.5, 0], (material.paramCols * 2 + 5) * 4);
    vi.mocked(readBuildingAlphaMaps).mockResolvedValueOnce({
      tint: { width: 2, height: 1, data: new Uint8Array([255, 255]) },
      shader: { width: 2, height: 1, data: new Uint8Array([255, 0]) },
    });
    const source = await analyzeBuildingMaterial({
      glbs: [
        await sourceTriangle(20, 7),
        await sourceTriangle(8, 5, false, 12, true),
      ],
      materials: [material],
      meshMaterialIndices: [0, 0],
      meshUvKinds: [2, 2],
      diagnostics: "",
    });
    expect(source.facade.column).toBe(7);
    expect(source.facade.windowCoverage).toBe(1);
    expect(source.facade.topCoverage).toBe(0);
  });

  it("falls back to source area if mask decoding fails", async () => {
    vi.mocked(readBuildingAlphaMaps).mockRejectedValueOnce(
      new Error("Invalid PNG"),
    );
    const source = await analyzeBuildingMaterial({
      glbs: [await sourceTriangle(20, 7), await sourceTriangle(8, 5)],
      materials: [fixtureMaterial()],
      meshMaterialIndices: [0, 0],
      meshUvKinds: [2, 2],
      diagnostics: "",
    });
    expect(source.facade.column).toBe(7);
    expect(source.facade.windowCoverage).toBe(0);
  });
});
