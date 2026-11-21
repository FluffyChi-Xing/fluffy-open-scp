import { afterEach, expect, it, vi } from "vitest";
import * as THREE from "three";
const mock = vi.hoisted(() => ({ compose: vi.fn() }));
vi.mock("@/pages/packages/components/property-editor/refinedGround", () => ({
  composeRefinedGround: mock.compose,
}));
import {
  applyGroundTextures,
  releaseGroundTextureCache,
} from "./groundTextures";
afterEach(() => {
  releaseGroundTextureCache();
  vi.restoreAllMocks();
  mock.compose.mockReset();
});
it("shares in-flight white-mode prewarming with refined rendering without changing the white material", async () => {
  vi.spyOn(THREE.TextureLoader.prototype, "loadAsync").mockResolvedValue(
    new THREE.Texture({ width: 1, height: 1 } as HTMLImageElement),
  );
  let finish!: (value: {
    albedo: THREE.Texture;
    normal: THREE.Texture;
  }) => void;
  mock.compose.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const ground = new THREE.Group(),
    material = new THREE.MeshBasicMaterial(),
    fill = new THREE.Mesh(new THREE.PlaneGeometry(), material);
  ground.add(fill);
  const options = {
    THREE,
    ground,
    maskPng: "mask",
    refined: true,
    lotColors: [[1, 2, 3, 0]] as [number, number, number, number][],
    baseTileIndex: 1,
    isStale: () => false,
  };
  const warm = applyGroundTextures({ ...options, prewarmOnly: true });
  await vi.waitFor(() => expect(mock.compose).toHaveBeenCalledOnce());
  expect(fill.material).toBe(material);
  const render = applyGroundTextures(options);
  await Promise.resolve();
  expect(mock.compose).toHaveBeenCalledOnce();
  finish({ albedo: new THREE.Texture(), normal: new THREE.Texture() });
  await Promise.all([warm, render]);
  expect(fill.material).toBeInstanceOf(THREE.MeshPhongMaterial);
});
