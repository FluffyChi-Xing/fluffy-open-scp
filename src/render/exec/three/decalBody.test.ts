import * as THREE from "three";
import { expect, it } from "vitest";
import { buildDecalBody, type DecalBodyInput } from "./decalBody";
const unit = {
  kind: "decal",
  index: 0,
  category: 1,
  scale: 2,
  depth: 1,
  materialData: [0, 0, 0],
  fields: [],
  transform: { matrix: [1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0] },
};
function input(authored: boolean): DecalBodyInput {
  return {
    THREE,
    unit: {
      ...unit,
      ...(authored
        ? {
            decalSource: {
              packagePath: "Game.package",
              atlas: { typeId: 1, group: 2, instance: 3 },
            },
          }
        : {}),
    },
    texture: { png: "data", materialInstance: 0x12345678, aspectRatio: 1 },
    decoded: new THREE.Texture(),
    proxies: [],
    env: {},
    engineMaterials: false,
    decalLight: null,
    powered: true,
    isStale: () => false,
    state: {
      projected: 0,
      fallback: 0,
      holeLightCount: 0,
      neonAnimated: false,
    },
  } as unknown as DecalBodyInput;
}
it("keeps a manually placed projected decal visible when no receiver is hit", async () => {
  const request = input(true);
  const object = await buildDecalBody(request);
  expect(object?.children).toHaveLength(1);
  expect(request.state.fallback).toBe(1);
  object!.updateMatrixWorld(true);
  expect(
    object!.children[0].getWorldPosition(new THREE.Vector3()).z,
  ).toBeCloseTo(0.02);
});
it("preserves projection-only behavior for native decals", async () => {
  const request = input(false);
  const object = await buildDecalBody(request);
  expect(object?.children).toHaveLength(0);
  expect(request.state.fallback).toBe(0);
});

it("keeps authored plane-family decals above a downward-facing drop surface", async () => {
  const base = input(true);
  const request = {
    ...base,
    texture: { ...base.texture, materialInstance: 0x73684efc },
  };
  const object = await buildDecalBody(request);
  object!.updateMatrixWorld(true);
  expect(object?.children).toHaveLength(1);
  expect(
    object!.children[0].getWorldPosition(new THREE.Vector3()).z,
  ).toBeCloseTo(0.02);
});
