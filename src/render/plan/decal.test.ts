import * as THREE from "three";
import { describe, expect, it } from "vitest";
import { decalFrame } from "@/lib/decalProject";
import { decalRoute } from "@/pages/packages/components/property-editor/decalRouting";
import { decalFramePlan, decalRoutePlan } from "./decal";

let seed = 0xdecaf123;
const random = () => {
  seed = (seed * 1664525 + 1013904223) >>> 0;
  return seed / 4294967296;
};
const randomMatrix = (): number[] => {
  const matrix = new THREE.Matrix4().compose(
    new THREE.Vector3((random() - 0.5) * 80, (random() - 0.5) * 80, random() * 10),
    new THREE.Quaternion().setFromEuler(
      new THREE.Euler((random() - 0.5) * 3, (random() - 0.5) * 3, (random() - 0.5) * 3),
    ),
    new THREE.Vector3(1, 1, 1),
  );
  const e = matrix.elements;
  return [e[0], e[1], e[2], e[4], e[5], e[6], e[8], e[9], e[10], e[12], e[13], e[14]];
};

describe("decalRoutePlan ↔ decalRoute", () => {
  it("6 个 materialInstance 路由一致", () => {
    for (const material of [0x73684efc, 0xe5390a98, 0x4491de3a, 0x12345678, null, undefined]) {
      expect(decalRoutePlan(material)).toEqual(decalRoute(material));
    }
  });
});

describe("decalFramePlan ↔ decalFrame", () => {
  it("15 组随机 frame 的 origin/axes/size 一致", () => {
    for (let i = 0; i < 15; i += 1) {
      const matrix = randomMatrix();
      const scale = 0.5 + random() * 12;
      const depth = random() * 6;
      const aspect = 0.4 + random() * 3;
      const legacy = decalFrame(
        THREE,
        { transform: { matrix }, scale, depth } as never,
        aspect,
      );
      const plan = decalFramePlan(matrix, scale, depth, aspect);
      expect(legacy).not.toBeNull();
      expect(plan).not.toBeNull();
      const expectVec = (actual: readonly number[], expected: THREE.Vector3) => {
        expect(actual[0]).toBeCloseTo(expected.x, 9);
        expect(actual[1]).toBeCloseTo(expected.y, 9);
        expect(actual[2]).toBeCloseTo(expected.z, 9);
      };
      expectVec(plan!.origin, legacy!.origin);
      expectVec(plan!.axisX, legacy!.axisX);
      expectVec(plan!.axisY, legacy!.axisY);
      expectVec(plan!.axisZ, legacy!.axisZ);
      expect(plan!.sizeX).toBeCloseTo(legacy!.sizeX, 9);
      expect(plan!.sizeY).toBeCloseTo(legacy!.sizeY, 9);
      expect(plan!.boxDepth).toBeCloseTo(Math.max(depth * scale, 0), 9);
    }
  });

  it("非法输入返回 null（与旧语义一致）", () => {
    expect(decalFramePlan(null, 1, 1, 1)).toBeNull();
    expect(decalFramePlan([1], 1, 1, 1)).toBeNull();
    expect(decalFramePlan(randomMatrix(), 0, 1, 1)).toBeNull();
    expect(decalFramePlan(randomMatrix(), Number.NaN, 1, 1)).toBeNull();
  });
});
