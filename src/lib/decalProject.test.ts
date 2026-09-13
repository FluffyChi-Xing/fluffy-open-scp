import { describe, expect, it } from "vitest";
import type * as ThreeNamespace from "three";
import type { DecalUnit } from "@/api/tauri";
import {
  decalFrame,
  decalProjector,
  measureAnchorDistance,
  projectDecal,
} from "./decalProject";

const THREE = (await import("three")) as typeof ThreeNamespace;

/** 单位阵 + 平移（WPF 行主序行向量：前 3 行基向量、末行平移）。 */
function transform(tx = 0, ty = 0, tz = 0): DecalUnit["transform"] {
  return { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, tx, ty, tz] };
}

function decal(overrides: Partial<DecalUnit> = {}): DecalUnit {
  return {
    kind: "decal",
    index: 0,
    category: 0,
    transform: transform(),
    scale: 2,
    depth: 0.2,
    materialData: null,
    fields: [],
    ...overrides,
  };
}

/** 轴对齐立方体，中心在 center、边长 size。 */
function boxMesh(center: [number, number, number], size: number) {
  const geometry = new THREE.BoxGeometry(size, size, size);
  geometry.translate(...center);
  return new THREE.Mesh(geometry);
}

describe("decalFrame", () => {
  it("reads the WPF row-major rows as the basis and scale as half-width", () => {
    const frame = decalFrame(THREE, decal({ scale: 3 }), 2);
    expect(frame).not.toBeNull();
    expect(frame!.origin.toArray()).toEqual([0, 0, 0]);
    // 只镜像 U 的约定依赖 axisZ 为 +Z
    expect(frame!.axisZ.toArray()).toEqual([0, 0, 1]);
    expect(frame!.sizeX).toBeCloseTo(6); // 2 × scale
    expect(frame!.sizeY).toBeCloseTo(3); // 2 × scale / aspect
  });

  it("bails out without a scale or a 12-float transform", () => {
    expect(decalFrame(THREE, decal({ scale: null }), 1)).toBeNull();
    expect(decalFrame(THREE, decal({ transform: null }), 1)).toBeNull();
    expect(
      decalFrame(THREE, decal({ transform: { matrix: [1, 0, 0] } }), 1),
    ).toBeNull();
  });
});

describe("measureAnchorDistance", () => {
  it("anchors on the nearest surface along +local Z", () => {
    const frame = decalFrame(THREE, decal({ scale: 2 }), 1)!;
    // 立方体近面在 z = 1（中心 6、边长 10）
    const mesh = boxMesh([0, 0, 6], 10);
    const anchor = measureAnchorDistance(THREE, frame, [mesh]);
    expect(anchor).toBeCloseTo(1, 1);
  });

  it("returns null when nothing is within range", () => {
    const frame = decalFrame(THREE, decal({ scale: 2 }), 1)!;
    const mesh = boxMesh([0, 0, 500], 10);
    expect(measureAnchorDistance(THREE, frame, [mesh])).toBeNull();
  });
});

describe("decalProjector", () => {
  it("centers the box on the anchor and keeps it thin", () => {
    const frame = decalFrame(THREE, decal({ scale: 2, depth: 0.2 }), 1)!;
    const { position, size } = decalProjector(THREE, frame, 4, 0.2);
    expect(position.toArray()).toEqual([0, 0, 4]);
    expect(size.x).toBeCloseTo(4);
    expect(size.y).toBeCloseTo(4);
    // depth 0.2 < 下限 0.5 ⇒ 半厚取 0.5
    expect(size.z).toBeCloseTo(1);
  });

  it("clamps the half-thickness into [0.5, 2]", () => {
    const frame = decalFrame(THREE, decal({ scale: 2, depth: 9 }), 1)!;
    expect(decalProjector(THREE, frame, 0, 9).size.z).toBeCloseTo(4);
  });
});

describe("projectDecal", () => {
  it("projects the decal onto the wall and keeps its UVs in-box", async () => {
    const frame = decalFrame(THREE, decal({ scale: 2 }), 1)!;
    const mesh = boxMesh([0, 0, 6], 10);
    const geometry = await projectDecal(THREE, frame, [mesh], 0.2);
    expect(geometry).not.toBeNull();
    const position = geometry!.attributes.position;
    expect(position.count).toBeGreaterThan(0);
    let uvMin = Infinity;
    let uvMax = -Infinity;
    const uv = geometry!.attributes.uv;
    for (let i = 0; i < position.count; i += 1) {
      // 近面在 z = 1，被 ±size/2 的薄盒夹住
      expect(position.getZ(i)).toBeGreaterThan(0.4);
      expect(position.getZ(i)).toBeLessThan(1.6);
      // 足迹是 4×4，裁剪后退化到面片上
      expect(Math.abs(position.getX(i))).toBeLessThanOrEqual(2.01);
      expect(Math.abs(position.getY(i))).toBeLessThanOrEqual(2.01);
      uvMin = Math.min(uvMin, uv.getX(i));
      uvMax = Math.max(uvMax, uv.getX(i));
    }
    expect(uvMax - uvMin).toBeGreaterThan(0.9);
  });

  it("returns null when the decal points at nothing", async () => {
    const frame = decalFrame(THREE, decal({ scale: 2 }), 1)!;
    const mesh = boxMesh([0, 0, 500], 10);
    expect(await projectDecal(THREE, frame, [mesh], 0.2)).toBeNull();
  });

  it("returns null without building meshes", async () => {
    const frame = decalFrame(THREE, decal({ scale: 2 }), 1)!;
    expect(await projectDecal(THREE, frame, [], 0.2)).toBeNull();
  });
});
