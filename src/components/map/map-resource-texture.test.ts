import { expect, it } from "vitest";
import type { Region3DData, StateSource } from "@/lib/region-map";
import { createEditedResourceTexture } from "./map-resource-texture";
import { stateResourcePixels } from "./map-state-fields";

it("reuses the overlay texture and limits paint and erase uploads to changed pixels", () => {
  const source = (site: string, x: number, values: number[]): StateSource => ({
    site, origin: [x, 0], bounds: [x, 0, x + 32, 32], curves: [],
    maps: [{ id: 0xd779c976, size: 2, values }],
  });
  const first = source("a", 0, [0, 0, 0, 0]);
  const second = source("b", 64, [65535, 0, 0, 0]);
  const fixture = (active: StateSource): Region3DData => ({
    size: 8, metersPerPixel: 16, originWorld: [0, 0],
    waterZ: 0, heightDiv: 1, heightBias: 0, desert: false,
    displayName: null, displayNameEn: null, heightPngBase64: "", groundPngBase64: "",
    plots: [], brushes: [], staticLayers: { sources: [active, second] },
  });
  const { texture, update } = createEditedResourceTexture("coal", 8);
  update(fixture(first));
  const pixels = texture.image.data!;
  expect(texture.updateRanges).toEqual([]);
  texture.onUpdate?.(texture);

  const painted = fixture(source("a", 0, [0, 65535, 0, 0]));
  update(painted);
  expect(texture.image.data).toBe(pixels);
  expect(texture.updateRanges).toEqual([{ start: 4, count: 4 }]);
  expect(pixels).toEqual(stateResourcePixels(painted, "coal", 8));
  texture.clearUpdateRanges();

  update(fixture(first));
  expect(texture.updateRanges).toEqual([{ start: 4, count: 4 }]);
  expect(pixels[4]).toBe(0);
  expect(pixels[7]).toBe(255);
  expect(pixels[16]).toBe(255);
});
