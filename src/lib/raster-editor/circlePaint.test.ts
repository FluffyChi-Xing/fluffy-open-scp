import { expect, it } from "vitest";
import { drawCircle } from "./circlePaint";
import { defaultDecalLayers, protectDecalPaint } from "./decalLayers";
import { RasterDocument } from "./document";

it("uses the given radius and respects outline width", () => {
  const doc = new RasterDocument(9, 9);
  const center = { x: 4, y: 4 };
  const color = [255, 0, 0, 0] as const;
  expect(drawCircle(doc, center, 4, 1, color, false, () => {})).toEqual({ x: 0, y: 0, w: 9, h: 9 });
  expect(doc.getPixel(4, 4)[0]).toBe(0);
  expect(doc.getPixel(4, 2)[0]).toBe(0);
  drawCircle(doc, center, 4, 2, color, false, () => {});
  expect(doc.getPixel(4, 2)[0]).toBe(255);
  expect(doc.getPixel(4, 4)[0]).toBe(0);
  drawCircle(doc, center, 4, 1, color, true, () => {});
  expect(doc.getPixel(4, 4)[0]).toBe(255);
  expect(doc.getPixel(0, 0)[0]).toBe(0);
});

it("clips a circle at the canvas edge and applies the existing layer protection", () => {
  const doc = new RasterDocument(5, 5);
  doc.setPixel(1, 1, [0, 255, 0, 0]);
  const layers = defaultDecalLayers.map((layer, index) => ({ ...layer, locked: index === 1 }));
  const plot = protectDecalPaint(doc, doc.pixels.slice(), layers, 3);
  drawCircle(doc, { x: 1, y: 1 }, 3, 1, [0, 0, 0, 255], true, plot);
  expect(doc.getPixel(1, 1)).toEqual([0, 255, 0, 0]);
  expect(doc.getPixel(2, 1)).toEqual([0, 0, 0, 255]);
});
