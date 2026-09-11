import { describe, expect, it } from "vitest";
import type * as ThreeNamespace from "three";
import { unitId, unitMatrix } from "./unitGizmos";
import {
  createUnitEditLayer,
  mergeUnitOverrides,
  threeToRowMajor,
  translateRowMajor,
} from "./unitEditLayer";
import type { LotUnitDto, PropUnit } from "@/api/tauri";

const THREE = (await import("three")) as typeof ThreeNamespace;

/** 组合一个非平凡 T·R·S 矩阵（含旋转与缩放）做 roundtrip。 */
function composedMatrix(): ThreeNamespace.Matrix4 {
  const rotation = new THREE.Quaternion().setFromEuler(
    new THREE.Euler(0.4, 0.9, -0.3),
  );
  return new THREE.Matrix4().compose(
    new THREE.Vector3(11, -7, 3.5),
    rotation,
    new THREE.Vector3(1.5, 0.75, 2.25),
  );
}

describe("threeToRowMajor", () => {
  it("roundtrips through unitMatrix without loss (写回命门)", () => {
    const matrix = composedMatrix();
    const rowMajor = threeToRowMajor(matrix);
    expect(rowMajor).toHaveLength(12);
    const back = unitMatrix(THREE, { matrix: rowMajor });
    for (let i = 0; i < 16; i += 1) {
      expect(back.elements[i]).toBeCloseTo(matrix.elements[i], 10);
    }
  });

  it("decomposes back to the same position/quaternion/scale", () => {
    const matrix = composedMatrix();
    const roundtrip = unitMatrix(THREE, {
      matrix: threeToRowMajor(matrix),
    });
    const position = new THREE.Vector3();
    const quaternion = new THREE.Quaternion();
    const scale = new THREE.Vector3();
    roundtrip.decompose(position, quaternion, scale);
    expect(position.x).toBeCloseTo(11, 10);
    expect(position.y).toBeCloseTo(-7, 10);
    expect(position.z).toBeCloseTo(3.5, 10);
    expect(scale.x).toBeCloseTo(1.5, 10);
    expect(scale.z).toBeCloseTo(2.25, 10);
    // 同一旋转（允许双覆盖 q 与 -q 等价）
    const expected = new THREE.Quaternion().setFromEuler(
      new THREE.Euler(0.4, 0.9, -0.3),
    );
    const dot = Math.abs(quaternion.dot(expected));
    expect(dot).toBeCloseTo(1, 10);
  });
});

describe("translateRowMajor", () => {
  it("adds the delta to the translation row only (世界空间平移)", () => {
    const matrix = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    const next = translateRowMajor(matrix, [0.5, -1, 2]);
    expect(next).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10.5, 10, 14]);
    // 原矩阵不被改动
    expect(matrix[9]).toBe(10);
  });

  it("matches a three.js world-space translation of the same matrix", () => {
    const original = composedMatrix();
    const moved = original.clone();
    moved.setPosition(
      original.elements[12] + 0.5,
      original.elements[13] - 1,
      original.elements[14] + 2,
    );
    const viaRowMajor = unitMatrix(THREE, {
      matrix: translateRowMajor(threeToRowMajor(original), [0.5, -1, 2]),
    });
    for (let i = 0; i < 16; i += 1) {
      expect(viaRowMajor.elements[i]).toBeCloseTo(moved.elements[i], 10);
    }
  });
});

describe("createUnitEditLayer", () => {
  it("applies, undoes and redoes transform overrides", () => {
    const layer = createUnitEditLayer();
    expect(layer.canUndo.value).toBe(false);
    layer.setUnitTransform("prop:0:1", [1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 2, 3]);
    expect(layer.overrides.get("prop:0:1")).toEqual([
      1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 2, 3,
    ]);
    expect(layer.canUndo.value).toBe(true);
    layer.setUnitTransform("prop:0:1", [1, 0, 0, 0, 1, 0, 0, 0, 1, 4, 5, 6]);
    expect(layer.overrides.get("prop:0:1")![9]).toBe(4);
    layer.undo();
    expect(layer.overrides.get("prop:0:1")![9]).toBe(1);
    layer.undo();
    expect(layer.overrides.has("prop:0:1")).toBe(false);
    expect(layer.canUndo.value).toBe(false);
    expect(layer.canRedo.value).toBe(true);
    layer.redo();
    expect(layer.overrides.get("prop:0:1")![9]).toBe(1);
  });

  it("rejects malformed matrices and resets cleanly", () => {
    const layer = createUnitEditLayer();
    layer.setUnitTransform("light:0", [1, 2, 3]);
    expect(layer.overrides.size).toBe(0);
    layer.setUnitTransform("light:0", [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
    expect(layer.editCount.value).toBe(1);
    layer.reset();
    expect(layer.overrides.size).toBe(0);
    expect(layer.canUndo.value).toBe(false);
    expect(layer.canRedo.value).toBe(false);
  });
});

describe("mergeUnitOverrides", () => {
  it("replaces the transform of overridden units and keeps others by reference", () => {
    const untouched = {
      kind: "prop",
      index: 0,
      bin: 0,
      transform: null,
      slot: null,
      fields: [],
    } as PropUnit;
    const edited = {
      kind: "prop",
      index: 1,
      bin: 0,
      transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0] },
      slot: null,
      fields: [],
    } as PropUnit;
    const overrides = new Map([
      [unitId(edited), [2, 0, 0, 0, 2, 0, 0, 0, 2, 7, 8, 9]],
    ]);
    const merged = mergeUnitOverrides(
      [untouched, edited] as LotUnitDto[],
      overrides,
    );
    expect(merged[0]).toBe(untouched);
    const mergedProp = merged[1] as PropUnit;
    expect(mergedProp.transform!.matrix).toEqual([2, 0, 0, 0, 2, 0, 0, 0, 2, 7, 8, 9]);
    // 原 DTO 不被改动
    expect(edited.transform!.matrix[0]).toBe(1);
  });

  it("returns the input list untouched when no overrides exist", () => {
    const units = [] as LotUnitDto[];
    expect(mergeUnitOverrides(units, new Map())).toBe(units);
  });
});
