import * as THREE from "three";
import { describe, expect, it } from "vitest";
import { unitMatrix } from "@/pages/packages/components/property-editor/unitGizmos";
import { rowMajor12ToTrs } from "./transform";

let seed = 0x12345678;
const random = () => {
  seed = (seed * 1664525 + 1013904223) >>> 0;
  return seed / 4294967296;
};
const randomRowMajor12 = (): number[] => {
  const quaternion = new THREE.Quaternion().setFromEuler(
    new THREE.Euler((random() - 0.5) * 3, (random() - 0.5) * 3, (random() - 0.5) * 3),
  );
  const matrix = new THREE.Matrix4().compose(
    new THREE.Vector3((random() - 0.5) * 80, (random() - 0.5) * 80, random() * 20),
    quaternion,
    new THREE.Vector3(0.5 + random() * 3, 0.5 + random() * 3, 0.5 + random() * 3),
  );
  const e = matrix.elements;
  return [e[0], e[1], e[2], e[4], e[5], e[6], e[8], e[9], e[10], e[12], e[13], e[14]];
};

describe("rowMajor12ToTrs ↔ three decompose", () => {
  it("25 组随机矩阵逐值一致（含缩放/旋转/平移）", () => {
    for (let i = 0; i < 25; i += 1) {
      const raw = randomRowMajor12();
      const legacy = unitMatrix(THREE, { matrix: raw } as never);
      const position = new THREE.Vector3();
      const quaternion = new THREE.Quaternion();
      const scale = new THREE.Vector3();
      legacy.decompose(position, quaternion, scale);
      const trs = rowMajor12ToTrs(raw);
      expect(trs).not.toBeNull();
      expect(trs!.position[0]).toBeCloseTo(position.x, 6);
      expect(trs!.position[1]).toBeCloseTo(position.y, 6);
      expect(trs!.position[2]).toBeCloseTo(position.z, 6);
      expect(trs!.scale[0]).toBeCloseTo(scale.x, 6);
      expect(trs!.scale[1]).toBeCloseTo(scale.y, 6);
      expect(trs!.scale[2]).toBeCloseTo(scale.z, 6);
      const dot = Math.abs(
        trs!.quaternion[0] * quaternion.x +
          trs!.quaternion[1] * quaternion.y +
          trs!.quaternion[2] * quaternion.z +
          trs!.quaternion[3] * quaternion.w,
      );
      expect(dot).toBeCloseTo(1, 6);
    }
  });

  it("非法输入返回 null（长度/非有限值）", () => {
    expect(rowMajor12ToTrs(null)).toBeNull();
    expect(rowMajor12ToTrs([])).toBeNull();
    expect(rowMajor12ToTrs(new Array(12).fill(Number.NaN))).toBeNull();
  });
});
