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

  it("灯位代理：Spot 默认体积光锥，显式 isVolumetric=false 才退回半透明截锥", () => {
    const light = prop({
      kind: "light",
      lightType: "Spot",
      color: [1, 0.8, 0.2],
      outerRadius: 3,
      innerRadius: 2,
      diffuse: 6,
      length: 2,
    });
    const group = buildUnitObject(THREE, light) as THREE.Group;
    const beam = group.getObjectByName("__lightVolume") as THREE.Mesh;
    expect(beam).toBeInstanceOf(THREE.Mesh);
    expect((beam.geometry as THREE.CylinderGeometry).type).toBe("CylinderGeometry");
    expect(beam.material).toMatchObject({
      transparent: true,
      blending: THREE.AdditiveBlending,
    });
    expect(group.getObjectByName("__lightBulb")).toBeInstanceOf(THREE.Mesh);

    const flat = buildUnitObject(
      THREE,
      prop({
        kind: "light",
        lightType: "Spot",
        outerRadius: 3,
        length: 2,
        isVolumetric: false,
      }),
    ) as THREE.Group;
    expect(flat.getObjectByName("__lightVolume")).toBeUndefined();
    const marker = flat.children.find(
      (child) => child.name !== "__lightBulb" && (child as THREE.Mesh).isMesh,
    ) as THREE.Mesh;
    expect((marker.material as THREE.MeshBasicMaterial).opacity).toBeCloseTo(0.16);
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
    for (const [lightType, distance] of [["Point", 80], ["Spot", 100]] as const) {
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
      expect((object.children[0] as THREE.PointLight | THREE.SpotLight).distance).toBe(distance);
    }
  });

  it.each(["Point", "Spot"])("精细模式 %s 的 Kd 不再按米数额外衰减", (lightType) => {
    const unit = prop({ kind: "light", lightType, diffuse: 0.8, length: 24 });
    const object = buildRealLightUnit(THREE, unit as never);
    const lights = object.children.filter(
      (child): child is THREE.PointLight | THREE.SpotLight =>
        child instanceof THREE.PointLight || child instanceof THREE.SpotLight,
    );
    expect(lights.length).toBeGreaterThan(0);
    expect(lights.every((light) => light.decay === 0)).toBe(true);
    expect(lights.reduce((sum, light) => sum + light.intensity, 0)).toBeCloseTo(0.8);
  });

  it.each(["Point", "Spot", "Line"])("精细模式 %s 不绘制灯具标记", (lightType) => {
    const unit = prop({ kind: "light", lightType, length: 12, diffuse: 0, transform: null });
    const group = buildRealLightUnit(THREE, unit as never);
    expect(group.getObjectByName("__lineLightEmitter")).toBeUndefined();
    expect(group.getObjectByName("__lightBulb")).toBeUndefined();
    if (lightType === "Line") {
      const defaultMarker = buildUnitObject(THREE, unit)!.getObjectByName("__lineLightEmitter") as THREE.Mesh;
      expect(defaultMarker.material).toBeInstanceOf(THREE.MeshBasicMaterial);
    }
  });

  it("线灯用一段圆柱光照，并独立保留 Kd 与 Ks", () => {
    const unit = prop({
      kind: "light", lightType: "Line", length: 24, outerRadius: 3,
      innerRadius: 1, diffuse: 0.8, specular: 0.4, falloffStart: 2,
    });
    const group = buildRealLightUnit(THREE, unit as never);
    expect(group.children.some((child) => child instanceof THREE.Light)).toBe(false);
    expect(group.userData.lineLight).toMatchObject({
      length: 24, radius: 3, innerRadius: 1, diffuse: 0.8, specular: 0.4,
    });
  });

  it("放置的 Spot 灯：length 缺失时仍有可见体积光锥", () => {
    // 旧实现 length=null → max(0, 0.05) = 5 cm 锥体，放置后直接看不见。
    const unit = prop({
      kind: "light",
      lightType: "Spot",
      outerRadius: 8,
      length: null,
      isVolumetric: true,
      transform: null,
    });
    const object = buildUnitObject(THREE, unit)!;
    object.updateMatrixWorld(true);
    const size = new THREE.Box3().setFromObject(object).getSize(new THREE.Vector3());
    expect(size.y).toBeGreaterThan(1);
    const beam = object.getObjectByName("__lightVolume") as THREE.Mesh | undefined;
    expect(beam).toBeInstanceOf(THREE.Mesh);
    expect(beam!.material).toMatchObject({
      transparent: true,
      blending: THREE.AdditiveBlending,
    });
  });

  it("放置的 Spot 灯：体积光锥不参与拾取，拾取落到灯球", () => {
    const unit = prop({
      kind: "light",
      lightType: "Spot",
      outerRadius: 8,
      length: 12,
      isVolumetric: true,
      transform: null,
    });
    const object = buildUnitObject(THREE, unit)!;
    object.updateMatrixWorld(true);
    const ray = new THREE.Raycaster(
      new THREE.Vector3(0, 6, 40),
      new THREE.Vector3(0, 0, -1),
    );
    // 射线穿过光锥中段；锥体 raycast 置空，只有灯球/可拾取件参与。
    expect(ray.intersectObject(object, true)).toHaveLength(0);
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
