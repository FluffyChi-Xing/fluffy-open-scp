import { describe, expect, it } from "vitest";
import type * as ThreeNamespace from "three";
import type { DecalUnit } from "@/api/tauri";
import {
  decalFrame,
  decalHalfThickness,
  decalProjector,
  snapQuadToSurface,
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

/** 单面片。facing="toward"：法线 -Z（迎着从原点出发的 +Z 射线，可被命中）；
 * facing="away"：法线 +Z（背面朝射线——Raycaster 按材质 FrontSide 剔除）。 */
function quadMesh(
  center: [number, number, number],
  facing: "toward" | "away",
) {
  const geometry = new THREE.PlaneGeometry(4, 4);
  if (facing === "toward") geometry.rotateY(Math.PI);
  geometry.translate(...center);
  return new THREE.Mesh(geometry);
}

describe("decalFrame", () => {
  it("reads the WPF row-major rows as the basis and scale as half-height", () => {
    const frame = decalFrame(THREE, decal({ scale: 3 }), 2);
    expect(frame).not.toBeNull();
    expect(frame!.origin.toArray()).toEqual([0, 0, 0]);
    expect(frame!.axisZ.toArray()).toEqual([0, 0, 1]);
    // 2026-09-27 OMEGACO 对照定谳：高 = 2×scale、宽 = 高×aspect
    expect(frame!.sizeY).toBeCloseTo(6); // 2 × scale
    expect(frame!.sizeX).toBeCloseTo(12); // 2 × scale × aspect
  });

  it("bails out without a scale or a 12-float transform", () => {
    expect(decalFrame(THREE, decal({ scale: null }), 1)).toBeNull();
    expect(decalFrame(THREE, decal({ transform: null }), 1)).toBeNull();
    expect(
      decalFrame(THREE, decal({ transform: { matrix: [1, 0, 0] } }), 1),
    ).toBeNull();
  });
});

describe("decalHalfThickness", () => {
  it("passes depth through and floors at a small epsilon", () => {
    expect(decalHalfThickness(1.056)).toBeCloseTo(1.056);
    expect(decalHalfThickness(9)).toBeCloseTo(9);
    expect(decalHalfThickness(0.2)).toBeCloseTo(0.2);
    expect(decalHalfThickness(null)).toBeCloseTo(0.05);
  });
});

describe("decalProjector", () => {
  it("centers the engine volume box on the transform origin", () => {
    // 盒 = 横向可见窗 2×scale×aspect、Z 全深 5×scale（基 0.5×10 实锤）
    const frame = decalFrame(THREE, decal({ scale: 2, depth: 0.2 }), 1)!;
    const { position, size } = decalProjector(THREE, frame);
    expect(position.toArray()).toEqual([0, 0, 0]);
    expect(size.x).toBeCloseTo(4);
    expect(size.y).toBeCloseTo(4);
    expect(size.z).toBeCloseTo(10); // 5 × scale
  });
});

describe("snapQuadToSurface", () => {
  it("snaps to the nearest surface inside the volume half-depth", () => {
    const frame = decalFrame(THREE, decal({ scale: 2 }), 1)!; // 半深 2.5
    // 墙面在 z = 1（-Z 法线朝向原点侧的射线）
    const wall = quadMesh([0, 0, 1], "toward");
    const { position, hit } = snapQuadToSurface(THREE, frame, [wall]);
    expect(hit).toBe(true);
    expect(position.z).toBeCloseTo(1 - 0.03, 2); // 贴面 - 3cm 回撤
    expect(position.x).toBeCloseTo(0);
    expect(position.y).toBeCloseTo(0);
  });

  it("prefers the nearer of two surfaces on opposite sides", () => {
    const frame = decalFrame(THREE, decal({ scale: 2 }), 1)!;
    const far = quadMesh([0, 0, 2], "toward");
    const near = quadMesh([0, 0, 0.5], "toward");
    const { position } = snapQuadToSurface(THREE, frame, [far, near]);
    expect(position.z).toBeCloseTo(0.5 - 0.03, 2);
  });

  it("stays at the origin when nothing is inside the volume", () => {
    const frame = decalFrame(THREE, decal({ scale: 2 }), 1)!; // 半深 2.5
    const wall = quadMesh([0, 0, 10], "toward"); // 体积盒之外
    const { position, hit } = snapQuadToSurface(THREE, frame, [wall]);
    expect(hit).toBe(false);
    expect(position.toArray()).toEqual([0, 0, 0]);
  });
});
