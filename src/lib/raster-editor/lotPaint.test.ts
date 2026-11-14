import { describe, expect, it } from "vitest";
import { lotPaintDisplay, lotRegion } from "./lotPaint";

describe("Lot paint semantics", () => {
  it("uses the game priority order and separates border bands", () => {
    expect(lotRegion([0, 0, 0, 255])).toBe(6);
    expect(lotRegion([0, 0, 255, 0])).toBe(4);
    expect(lotRegion([0, 0, 128, 0], [0, 0, 0.1, 0])).toBe(5);
  });

  it("renders independent tint colors without changing the mask", () => {
    const pixels = new Uint8ClampedArray([255, 0, 0, 0, 0, 0, 0, 255]);
    expect([...lotPaintDisplay(pixels, [[12, 34, 56], [0, 0, 0], [0, 0, 0], [78, 90, 12]])])
      .toEqual([12, 34, 56, 255, 78, 90, 12, 255]);
  });
});
