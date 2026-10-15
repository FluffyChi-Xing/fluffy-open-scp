import { describe, expect, it, vi } from "vitest";
import type * as Three from "three";
import { applyGroundMask } from "./editorGround";
const composeRefinedGround = vi.hoisted(() => vi.fn());
vi.mock("./refinedGround", () => ({ composeRefinedGround }));

describe("default Lot ground", () => {
  it("uses the authored albedo even when raw mask and normal atlas are available", async () => {
    const texture = { colorSpace: "", flipY: true };
    const loadAsync = vi.fn().mockResolvedValue(texture);
    const THREE = {
      SRGBColorSpace: "srgb",
      TextureLoader: class { loadAsync = loadAsync; },
    } as unknown as typeof Three;
    const material = {
      map: null,
      transparent: true,
      opacity: 0.08,
      color: { set: vi.fn() },
      needsUpdate: false,
    };
    const ground = { children: [{ isMesh: true, material }] } as unknown as Three.Object3D;
    await applyGroundMask({
      THREE,
      ground,
      maskPng: "data:image/png;base64,mask-default-test",
      albedoPng: "data:image/png;base64,albedo-default-test",
      refined: false,
      lotColors: [[0, 0, 0, 0], [255, 0, 0, 0], [0, 255, 0, 0], [0, 0, 255, 0]],
      baseTileIndex: 8,
      rawMask: { width: 1, height: 1, data: new Uint8ClampedArray(4) } as ImageData,
      normalAtlas: { width: 4, height: 4, data: new Uint8ClampedArray(64) } as ImageData,
      isStale: () => false,
    });
    expect(loadAsync).toHaveBeenCalledWith("data:image/png;base64,albedo-default-test");
    expect(material.map).toBe(texture);
    expect(material.transparent).toBe(false);
    expect(material.opacity).toBe(1);
  });

  it("keeps color blocks in default mode even when game atlases are present", async () => {
    composeRefinedGround.mockClear();
    const flatTexture = { colorSpace: "", flipY: true };
    const loadAsync = vi.fn().mockResolvedValue(flatTexture);
    const material = {
      map: null, transparent: true, opacity: 0.08,
      color: { set: vi.fn() }, needsUpdate: false,
    };
    const THREE = {
      SRGBColorSpace: "srgb",
      TextureLoader: class { loadAsync = loadAsync; },
    } as unknown as typeof Three;
    const ground = { children: [{ isMesh: true, material }] } as unknown as Three.Object3D;
    await applyGroundMask({
      THREE, ground, maskPng: "mask-with-detail", albedoPng: "flat-albedo",
      refined: false, lotColors: [[0, 0, 0, 0]], baseTileIndex: 8,
      surface: { width: 4, height: 4, data: new Uint8ClampedArray(64) } as ImageData,
      normalAtlas: { width: 4, height: 4, data: new Uint8ClampedArray(64) } as ImageData,
      isStale: () => false,
    });
    expect(loadAsync).toHaveBeenCalledWith("flat-albedo");
    expect(loadAsync).not.toHaveBeenCalledWith("mask-with-detail");
    expect(composeRefinedGround).not.toHaveBeenCalled();
    expect(material.map).toBe(flatTexture);
    expect(material.transparent).toBe(false);
    expect(material.opacity).toBe(1);
  });
});
