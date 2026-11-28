import * as THREE from "three";
import { afterEach, expect, it, vi } from "vitest";
import { registerTextureUniforms } from "./materialTextureInputs";
import {
  collectSceneTextures,
  primeTexturesBeforeDraw,
} from "./viewerTexturePriming";
afterEach(() => vi.restoreAllMocks());
function fixture() {
  const scene = new THREE.Scene(),
    map = new THREE.Texture(),
    tint = new THREE.Texture(),
    decal = new THREE.Texture();
  const building = new THREE.MeshStandardMaterial({ map });
  registerTextureUniforms(building, {
    tint: { value: tint },
    number: { value: 5 },
  });
  scene.add(new THREE.Mesh(new THREE.BoxGeometry(), building));
  scene.add(
    new THREE.Mesh(
      new THREE.PlaneGeometry(),
      new THREE.ShaderMaterial({
        uniforms: { decal: { value: decal }, shared: { value: [map, tint] } },
      }),
    ),
  );
  const primed = new Set<THREE.Texture>();
  return {
    scene,
    map,
    tint,
    decal,
    building,
    primed,
    isPrimed: (t: THREE.Texture) => primed.has(t),
    primeTexture: vi.fn((t: THREE.Texture) => {
      primed.add(t);
    }),
    isDisposed: () => false,
  };
}
it("collects and deduplicates standard, custom tint and shader-array textures", () => {
  const host = fixture();
  expect(collectSceneTextures(host)).toEqual([host.map, host.tint, host.decal]);
  host.primed.add(host.tint);
  expect(collectSceneTextures(host)).toEqual([host.map, host.decal]);
  expect(host.building.clone().userData).toEqual({});
});
it("yields between upload batches and cancels stale work before the next upload", async () => {
  const host = fixture();
  let clock = 0,
    stale = false;
  vi.spyOn(performance, "now").mockImplementation(() => clock);
  host.primeTexture.mockImplementation((t) => {
    host.primed.add(t);
    clock += 7;
  });
  const yieldTask = vi.fn(async () => {
    stale = true;
  });
  await primeTexturesBeforeDraw(host, () => stale, yieldTask);
  expect(yieldTask).toHaveBeenCalledOnce();
  expect(host.primeTexture).toHaveBeenCalledOnce();
  expect(host.primed.has(host.map)).toBe(true);
});
