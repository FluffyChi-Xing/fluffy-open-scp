import * as THREE from "three";
import { describe, expect, it } from "vitest";
import {
  buildLotDimensions,
  buildLotRect,
  placementInverse,
} from "@/pages/packages/components/property-editor/editorGround";
import { lotDimensionPlan, lotGroundPlan } from "./ground";
import { affineToColumnMajor16 } from "../resolve/matrix";

let seed = 0x5eed1234;
const random = () => {
  seed = (seed * 1664525 + 1013904223) >>> 0;
  return seed / 4294967296;
};
const randomPlacement = (): number[] => {
  const matrix = new THREE.Matrix4().compose(
    new THREE.Vector3((random() - 0.5) * 60, (random() - 0.5) * 60, 0),
    new THREE.Quaternion().setFromEuler(new THREE.Euler(0, 0, (random() - 0.5) * 3)),
    new THREE.Vector3(1, 1, 1),
  );
  const e = matrix.elements;
  return [e[0], e[1], e[2], e[4], e[5], e[6], e[8], e[9], e[10], e[12], e[13], e[14]];
};

const expectCloseArray = (actual: ArrayLike<number>, expected: readonly number[]) => {
  expect(actual.length).toBe(expected.length);
  for (let i = 0; i < expected.length; i += 1) {
    expect(actual[i]).toBeCloseTo(expected[i], 4);
  }
};

describe("lotGroundPlan ↔ editorGround", () => {
  it("placement 逆矩阵与旧 placementInverse 逐元素一致", () => {
    for (let i = 0; i < 12; i += 1) {
      const placement = randomPlacement();
      const plan = lotGroundPlan([32, 24], placement);
      expect(plan).not.toBeNull();
      const legacy = placementInverse(THREE, placement);
      expectCloseArray(affineToColumnMajor16(plan!.placementInverse!), Array.from(legacy.elements));
    }
  });

  it("矩形角点/尺寸与旧 buildLotRect 一致", () => {
    const plan = lotGroundPlan([48, 36], null)!;
    const legacy = buildLotRect(THREE, [48, 36]);
    const border = legacy.children[0] as THREE.LineLoop;
    const positions = border.geometry.attributes.position.array as Float32Array;
    expectCloseArray(positions, plan.corners.flat());
    const fill = legacy.children[1] as THREE.Mesh;
    expect((fill.geometry as THREE.PlaneGeometry).parameters.width).toBe(plan.width);
    expect((fill.geometry as THREE.PlaneGeometry).parameters.height).toBe(plan.depth);
    expect(fill.position.z).toBe(plan.fillZ);
  });

  it("无 placement / 非法尺寸返回 null", () => {
    expect(lotGroundPlan(null, null)).toBeNull();
    expect(lotGroundPlan([0, 10], null)).toBeNull();
  });
});

describe("lotDimensionPlan ↔ buildLotDimensions", () => {
  it("线段点位与锚点逐值一致（含高度）", () => {
    const plan = lotDimensionPlan([32, 24], 9.5)!;
    const legacy = buildLotDimensions(THREE, [32, 24], 9.5)!;
    const positions = legacy.object.geometry.attributes.position.array as Float32Array;
    expectCloseArray(positions, plan.points.flat());
    expect(plan.anchors).toEqual(legacy.anchors);
  });

  it("无高度时只有长/宽两个锚点", () => {
    const plan = lotDimensionPlan([32, 24], 0)!;
    expect(plan.anchors.map((anchor) => anchor.axis)).toEqual(["length", "width"]);
  });

  it("非法尺寸返回 null", () => {
    expect(lotDimensionPlan([0, 10], 3)).toBeNull();
  });
});
