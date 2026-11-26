import * as THREE from "three";
import { describe, expect, it, vi } from "vitest";
import type { DecalUnit, EffectDefinitionData, LotUnitDto } from "@/api/tauri";
import type { UnitRenderContext } from "@/pages/packages/components/property-editor/renderers/types";
import { kindGroup, stageDecals, stageUnits } from "./stages";
import { buildUnitObject, EFFECT_COLOR, SPAWNER_COLOR } from "../exec/three/overlays";

const unit = (kind: string, index: number, extra: Record<string, unknown> = {}) =>
  ({ kind, index, transform: null, fields: [], ...extra }) as unknown as LotUnitDto;

function groups() {
  const map = new Map<string, THREE.Group>();
  return (name: string) => {
    if (!map.has(name)) map.set(name, new THREE.Group());
    return map.get(name)!;
  };
}

const mesh = () => new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshBasicMaterial());

describe("kindGroup", () => {
  it("maps every unit kind to its viewport group", () => {
    expect(kindGroup("light")).toBe("lights");
    expect(kindGroup("prop")).toBe("props");
    expect(kindGroup("decal")).toBe("decals");
    expect(kindGroup("effect")).toBe("effects");
    expect(kindGroup("spawner")).toBe("spawners");
    expect(kindGroup("pathPoint")).toBe("paths");
    expect(kindGroup("model")).toBe("model");
  });
});

describe("stageUnits", () => {
  it("分派灯光/资产/降级并按组登记，贴花被跳过", async () => {
    const groupFor = groups();
    const unitObjects = new Map<string, THREE.Object3D>();
    const renderAssetUnit = vi.fn(async (_ctx, candidate) =>
      candidate.kind === "prop" ? mesh() : null,
    );
    const buildFallback = vi.fn(() => mesh());
    const buildLightUnit = vi.fn(() => mesh());
    const units = [
      unit("light", 0),
      unit("prop", 1),
      unit("spawner", 2),
      unit("effect", 3),
      unit("pathPoint", 4),
      unit("decal", 5),
    ];
    await stageUnits({
      THREE,
      units,
      renderMode: "refined",
      renderEffects: false,
      effectsCount: 0,
      assetCtx: {} as UnitRenderContext,
      particleViews: [],
      createParticleEffect: () => null,
      renderAssetUnit,
      buildFallback,
      buildLightUnit,
      groupFor,
      unitObjects,
      unitIdOf: (u) => `${u.kind}:${u.index}`,
      isStale: () => false,
      yieldToBrowser: async () => {},
    });
    expect(buildLightUnit).toHaveBeenCalledTimes(1);
    expect(renderAssetUnit).toHaveBeenCalledTimes(1);
    // spawner/effect/pathPoint 走降级；decal 不进入本阶段
    expect(buildFallback).toHaveBeenCalledTimes(3);
    expect(unitObjects.size).toBe(5);
    expect(groupFor("lights").children).toHaveLength(1);
    expect(groupFor("props").children).toHaveLength(1);
    expect(groupFor("spawners").children).toHaveLength(1);
    expect(groupFor("effects").children).toHaveLength(1);
    expect(groupFor("paths").children).toHaveLength(1);
    expect(groupFor("decals").children).toHaveLength(0);
    expect(unitObjects.get("prop:1")?.userData.unitKind).toBe("prop");
  });

  it.each([
    { renderMode: "default", renderEffects: true, particles: false },
    { renderMode: "refined", renderEffects: false, particles: false },
    { renderMode: "refined", renderEffects: true, particles: true },
  ] as const)("assembles the correct marker and effect for %j", async ({ renderMode, renderEffects, particles }) => {
    const effectId = { typeId: 1, group: 2, instance: 3 };
    const definition = { emitters: [{}] } as EffectDefinitionData;
    const unitObjects = new Map<string, THREE.Object3D>();
    const particle = { object: new THREE.Group(), tick: vi.fn(), dispose: vi.fn() };
    const createParticleEffect = vi.fn(() => particle);
    const renderAssetUnit = vi.fn(async () => mesh());
    await stageUnits({
      THREE,
      units: [unit("spawner", 1), unit("effect", 2, { effectId })],
      renderMode,
      renderEffects,
      effectDefinitions: new Map([[effectId.instance, definition]]),
      effectsCount: 1,
      assetCtx: {} as UnitRenderContext,
      particleViews: [],
      createParticleEffect,
      renderAssetUnit,
      buildFallback: buildUnitObject,
      buildLightUnit: () => mesh(),
      groupFor: groups(),
      unitObjects,
      unitIdOf: (u) => `${u.kind}:${u.index}`,
      isStale: () => false,
      yieldToBrowser: async () => {},
    });
    function expectCone(id: string, color: number) {
      const marker = unitObjects.get(id) as THREE.Mesh<THREE.CylinderGeometry, THREE.MeshStandardMaterial>;
      expect(marker.geometry.parameters).toMatchObject({ radiusTop: 1, radiusBottom: 0.001 });
      expect(marker.material.color.getHex()).toBe(color);
    }
    expect(renderAssetUnit).not.toHaveBeenCalled();
    expectCone("spawner:1", SPAWNER_COLOR);
    expect(createParticleEffect).toHaveBeenCalledTimes(particles ? 1 : 0);
    if (particles) expect(unitObjects.get("effect:2")).toBe(particle.object);
    else expectCone("effect:2", EFFECT_COLOR);
  });

  it("stale 时中止后续单元", async () => {
    let stale = false;
    const groupFor = groups();
    const unitObjects = new Map<string, THREE.Object3D>();
    await stageUnits({
      THREE,
      units: [unit("prop", 0), unit("prop", 1)],
      renderMode: "refined",
      renderEffects: false,
      effectsCount: 0,
      assetCtx: {} as UnitRenderContext,
      particleViews: [],
      createParticleEffect: () => null,
      renderAssetUnit: async () => {
        stale = true;
        return mesh();
      },
      buildFallback: () => mesh(),
      buildLightUnit: () => mesh(),
      groupFor,
      unitObjects,
      unitIdOf: (u) => `${u.kind}:${u.index}`,
      isStale: () => stale,
      yieldToBrowser: async () => {},
    });
    expect(unitObjects.size).toBe(1);
  });
});

describe("stageDecals", () => {
  const texture = { category: 0, index: 0, png: "x" } as never;

  it("精细模式走 buildDecalObject；缺失贴图时告警并降级", async () => {
    const groupFor = groups();
    const unitObjects = new Map<string, THREE.Object3D>();
    const buildDecalObject = vi.fn(async () => mesh());
    const buildFallback = vi.fn(() => mesh());
    const onMissingTexture = vi.fn();
    await stageDecals({
      THREE,
      renderMode: "refined",
      decalUnits: [
        unit("decal", 0, { category: 0 }),
        unit("decal", 1, { category: 0 }),
      ] as unknown as DecalUnit[],
      decalTextureByKey: new Map([["0:0", texture]]),
      buildingMeshes: [],
      proxies: [],
      buildDecalObject,
      buildFallback,
      groupFor,
      unitObjects,
      unitIdOf: (u) => `decal:${u.index}`,
      isStale: () => false,
      onMissingTexture,
    });
    expect(buildDecalObject).toHaveBeenCalledTimes(1);
    expect(buildFallback).toHaveBeenCalledTimes(1);
    expect(onMissingTexture).toHaveBeenCalledTimes(1);
    expect(groupFor("decals").children).toHaveLength(2);
  });

  it("默认模式一律降级标记锥", async () => {
    const buildDecalObject = vi.fn(async () => mesh());
    const buildFallback = vi.fn(() => mesh());
    await stageDecals({
      THREE,
      renderMode: "default",
      decalUnits: [unit("decal", 0, { category: 0 })] as unknown as DecalUnit[],
      decalTextureByKey: new Map([["0:0", texture]]),
      buildingMeshes: [],
      proxies: [],
      buildDecalObject,
      buildFallback,
      groupFor: groups(),
      unitObjects: new Map(),
      unitIdOf: (u) => `decal:${u.index}`,
      isStale: () => false,
      onMissingTexture: () => {},
    });
    expect(buildDecalObject).not.toHaveBeenCalled();
    expect(buildFallback).toHaveBeenCalledTimes(1);
  });
});
