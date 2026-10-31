import * as THREE from "three";
import { describe, expect, it } from "vitest";
import type { LotUnitDto } from "@/api/tauri";
import {
  EFFECT_COLOR,
  PROP_COLOR,
  SPAWNER_COLOR,
  buildPathLine,
  buildRealLightUnit,
  buildUnitObject,
} from "./overlays";
import { rowMajor12ToTrs } from "../../resolve/transform";

const rotation = (() => {
  const m = new THREE.Matrix4().compose(
    new THREE.Vector3(3, -2, 4),
    new THREE.Quaternion().setFromEuler(new THREE.Euler(0.2, 0.4, 0.6)),
    new THREE.Vector3(1.2, 1.2, 1.2),
  );
  const e = m.elements;
  return [e[0], e[1], e[2], e[4], e[5], e[6], e[8], e[9], e[10], e[12], e[13], e[14]];
})();

const transform = { matrix: rotation };

function prop(overrides: Record<string, unknown> = {}): LotUnitDto {
  return {
    kind: "prop",
    index: 1,
    bin: 2,
    transform,
    fields: [],
    ...overrides,
  } as unknown as LotUnitDto;
}

describe("overlays 构建器行为", () => {
  it("标记锥：颜色按 kind 区分，并应用 unit 变换", () => {
    const cases: [string, number][] = [
      ["prop", PROP_COLOR],
      ["effect", EFFECT_COLOR],
      ["spawner", SPAWNER_COLOR],
    ];
    for (const [kind, color] of cases) {
      const object = buildUnitObject(THREE, prop({ kind })) as THREE.Mesh;
      expect(object.type).toBe("Mesh");
      expect((object.material as THREE.MeshStandardMaterial).color.getHex()).toBe(color);
      const trs = rowMajor12ToTrs(rotation)!;
      expect(object.position.x).toBeCloseTo(trs.position[0], 5);
      expect(object.position.y).toBeCloseTo(trs.position[1], 5);
      expect(object.position.z).toBeCloseTo(trs.position[2], 5);
      expect(object.userData.unitKind).toBe(kind);
    }
  });

  it("灯位代理：Spot 用截锥且半透明", () => {
    const light = prop({
      kind: "light",
      lightType: "Spot",
      color: [1, 0.8, 0.2],
      outerRadius: 3,
      innerRadius: 2,
      diffuse: 6,
      length: 2,
    });
    const object = buildUnitObject(THREE, light) as THREE.Mesh;
    expect((object.geometry as THREE.CylinderGeometry).type).toBe("CylinderGeometry");
    expect((object.material as THREE.MeshStandardMaterial).opacity).toBeCloseTo(0.6);
  });

  it("贴花矩形：前后两片、背面使用提示色", () => {
    const decal = prop({ kind: "decal", category: 0, scale: 2.5 });
    const group = buildUnitObject(THREE, decal) as THREE.Group;
    expect(group.children).toHaveLength(2);
    expect(
      ((group.children[1] as THREE.Mesh).material as THREE.MeshStandardMaterial).color.getHex(),
    ).toBe(0x33cc33);
  });

  it("路径点：位置来自 point 字段而非 transform", () => {
    const pathPoint = prop({ kind: "pathPoint", point: [10, 20, 0], pointIndex: 0 });
    const mesh = buildUnitObject(THREE, pathPoint) as THREE.Mesh;
    expect(mesh.position.toArray()).toEqual([10, 20, 0]);
  });

  it("真实光源：只让拾取代理命中射线，Proxy 体积受控", () => {
    for (const lightType of ["Point", "Spot"]) {
      const unit = prop({
        kind: "light",
        lightType,
        outerRadius: 80,
        length: 100,
        transform: null,
      });
      const object = buildRealLightUnit(THREE, unit as never);
      object.updateMatrixWorld(true);
      const ray = new THREE.Raycaster(new THREE.Vector3(10, 0, 200), new THREE.Vector3(0, 0, -1));
      expect(ray.intersectObject(object, true)).toHaveLength(0);
      ray.ray.origin.x = 0;
      expect(ray.intersectObject(object, true).length).toBeGreaterThan(0);
      const proxy = object.children.find((child) => child.userData.pickOnly) as THREE.Mesh;
      expect(new THREE.Box3().setFromObject(proxy).getSize(new THREE.Vector3()).length()).toBeLessThan(4);
      expect((object.children[0] as THREE.PointLight).distance).toBe(160);
    }
  });

  it("路径折线：少于两点返回 null", () => {
    expect(buildPathLine(THREE, [new THREE.Vector3()])).toBeNull();
    const line = buildPathLine(THREE, [
      new THREE.Vector3(0, 0, 0),
      new THREE.Vector3(1, 2, 0),
    ])!;
    expect(line.type).toBe("Line");
    expect(
      (line.geometry as THREE.BufferGeometry).attributes.position.count,
    ).toBe(2);
  });
});
