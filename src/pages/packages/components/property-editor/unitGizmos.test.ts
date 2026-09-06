import { describe, expect, it } from "vitest";
import type * as ThreeNamespace from "three";
import { unitId, unitMatrix } from "./unitGizmos";
import type { DecalUnit, PropUnit, SpawnerUnit } from "@/api/tauri";

const THREE = (await import("three")) as typeof ThreeNamespace;

describe("unitMatrix", () => {
  it("maps WPF row-major translation into the three.js translation column", () => {
    // 行主序 [M11 M12 M13 | M21 M22 M23 | M31 M32 M33 | Tx Ty Tz]
    const matrix = unitMatrix(THREE, {
      matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 3, 4, 5],
    });
    const elements = matrix.elements;
    expect(elements[12]).toBe(3);
    expect(elements[13]).toBe(4);
    expect(elements[14]).toBe(5);
  });

  it("transposes the 3x3 basis (WPF row-vector → three column-vector)", () => {
    const matrix = unitMatrix(THREE, {
      matrix: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
    });
    const elements = matrix.elements;
    // elements 列主序：elements[1]=n21、elements[4]=n12。
    // 转置后 n12 = M21 = 4、n21 = M12 = 2。
    expect(elements[0]).toBe(1);
    expect(elements[1]).toBe(2);
    expect(elements[4]).toBe(4);
    expect(elements[6]).toBe(6);
    expect(elements[10]).toBe(9);
  });

  it("decomposes scale from a diagonal basis", () => {
    const matrix = unitMatrix(THREE, {
      matrix: [2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0],
    });
    const position = new THREE.Vector3();
    const quaternion = new THREE.Quaternion();
    const scale = new THREE.Vector3();
    matrix.decompose(position, quaternion, scale);
    expect([scale.x, scale.y, scale.z]).toEqual([2, 3, 4]);
  });

  it("falls back to identity for malformed matrices", () => {
    const matrix = unitMatrix(THREE, { matrix: [1, 2, 3] });
    expect(matrix.elements[0]).toBe(1);
    expect(matrix.elements[12]).toBe(0);
  });
});

describe("unitId", () => {
  it("carries bin/category dimensions for prop and decal units", () => {
    const prop = {
      kind: "prop",
      index: 2,
      bin: 13,
      transform: null,
      slot: null,
      fields: [],
    } as PropUnit;
    const decal = {
      kind: "decal",
      index: 1,
      category: 2,
      transform: null,
      scale: null,
      depth: null,
      materialData: null,
      fields: [],
    } as DecalUnit;
    const spawner = {
      kind: "spawner",
      index: 0,
      transform: null,
      id: null,
      fields: [],
    } as SpawnerUnit;
    expect(unitId(prop)).toBe("prop:13:2");
    expect(unitId(decal)).toBe("decal:2:1");
    expect(unitId(spawner)).toBe("spawner:0");
  });
});
