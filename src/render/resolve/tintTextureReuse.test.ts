import { afterEach, expect, it, vi } from "vitest";
import * as THREE from "three";
import type { LotModelPayload } from "@/api/tauri";
import { loadTintTextures } from "@/pages/packages/components/property-editor/refinedRender";
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it("reuses decoded tint textures across materials without mixing palette and interior sampling", async () => {
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:test");
  vi.stubGlobal("createImageBitmap", undefined);
  const load = vi.spyOn(THREE.ImageBitmapLoader.prototype, "load").mockImplementation(function (_url, onLoad) {
    onLoad?.({ width: 1, height: 1 } as ImageBitmap);
    return undefined;
  });
  const material = () => ({
    tintPng: new Uint8Array([1, 2, 3]), palettePng: new Uint8Array([1, 2, 3]),
    normalPng: new Uint8Array([1, 2, 3]), shaderPng: new Uint8Array([1, 2, 3]),
    interiorPng: new Uint8Array([1, 2, 3]), paramsF32: null, paramCols: 0,
  }) as NonNullable<LotModelPayload["materials"]>[number];
  const sets = await loadTintTextures(THREE, [material(), material()], 8, []);
  expect(load).toHaveBeenCalledTimes(3); // data, palette, interior
  expect(sets[0].tintTex).toBe(sets[1].tintTex);
  expect(sets[0].paletteTex).not.toBe(sets[0].tintTex);
  expect(sets[0].interiorTex).not.toBe(sets[0].paletteTex);
  expect(sets[0].tintTex?.colorSpace).toBe(THREE.NoColorSpace);
  expect(sets[0].tintTex?.wrapS).toBe(THREE.ClampToEdgeWrapping);
  expect(sets[0].paletteTex?.minFilter).toBe(THREE.NearestFilter);
  expect(sets[0].interiorTex?.colorSpace).toBe(THREE.SRGBColorSpace);
  expect(sets[0].interiorTex?.wrapS).toBe(THREE.RepeatWrapping);
});
