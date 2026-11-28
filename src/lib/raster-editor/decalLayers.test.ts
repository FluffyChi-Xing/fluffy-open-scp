import { describe, expect, it } from "vitest";
import { RasterDocument } from "./document";
import {
  createDecalFillGesture, defaultDecalLayers, protectDecalPaint, stampDecalLettering, visibleDecalPixels,
} from "./decalLayers";

function layers(locked: number | null, hidden: number | null = null) {
  return defaultDecalLayers.map((_, index) => ({ locked: index === locked, visible: index !== hidden }));
}

describe("decal paint layers", () => {
  it.each([0, 510])("stops at a locked visible closed outline with tolerance %s", (tolerance) => {
    const doc = new RasterDocument(9, 9);
    for (let y = 0; y < 9; y++)
      for (let x = 0; x < 9; x++)
        if (Math.abs(x - 4) + Math.abs(y - 4) === 3) doc.setPixel(x, y, [0, 0, 0, 255]);
    const paint = createDecalFillGesture(doc, doc.pixels.slice(), layers(3), 0, [255, 0, 0, 0], tolerance);
    paint({ x: 4, y: 4 });
    expect(doc.getPixel(4, 4)).toEqual([255, 0, 0, 0]);
    expect(doc.getPixel(0, 0)).toEqual([0, 0, 0, 0]);
    expect(doc.getPixel(4, 1)).toEqual([0, 0, 0, 255]);
    paint({ x: 4, y: 1 });
    expect(doc.getPixel(4, 1)).toEqual([0, 0, 0, 255]);
  });

  it("allows a lower layer to fill behind locked paint and cover other unlocked paint", () => {
    const doc = new RasterDocument(3, 1, Uint8ClampedArray.of(
      0, 0, 0, 0, 0, 0, 180, 220, 0, 0, 0, 0,
    ));
    const paint = createDecalFillGesture(doc, doc.pixels.slice(), layers(3), 0, [255, 0, 0, 0], 0, true);
    paint({ x: 0, y: 0 });
    expect([...doc.pixels]).toEqual([255, 0, 0, 0, 0, 0, 180, 220, 0, 0, 0, 0]);
    paint({ x: 2, y: 0 });
    expect([...doc.pixels]).toEqual([255, 0, 0, 0, 255, 0, 0, 220, 255, 0, 0, 0]);
  });

  it("does not cover a locked lower layer even with a higher active layer", () => {
    const doc = new RasterDocument(2, 1, Uint8ClampedArray.of(180, 0, 0, 0, 0, 220, 0, 0));
    const before = doc.pixels.slice();
    const glyphs = Uint8ClampedArray.of(0, 0, 0, 255, 0, 0, 0, 255);
    stampDecalLettering(doc, before, glyphs, layers(0), 3, [0, 0, 0, 255]);
    expect([...doc.pixels]).toEqual([180, 0, 0, 0, 0, 0, 0, 255]);
  });

  it("erases only the selected layer and filters hidden channels from export without losing paint", () => {
    const doc = new RasterDocument(1, 1, Uint8ClampedArray.of(180, 0, 195, 220));
    const before = doc.pixels.slice();
    const state = layers(null, 3);
    const protect = protectDecalPaint(doc, before, state, 2, true);
    doc.setPixel(0, 0, [0, 0, 0, 0]);
    protect(0, 0);
    expect([...doc.pixels]).toEqual([180, 0, 0, 220]);
    expect([...visibleDecalPixels(doc.pixels, state)]).toEqual([180, 0, 0, 0]);
    expect([...doc.pixels]).toEqual([180, 0, 0, 220]);
  });
});
