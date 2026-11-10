import { describe, expect, it } from "vitest";
import {
  clearSlotPixels,
  firstFreeSlot,
  groundDisplayPixels,
  lotColorsFromSlots,
  maskSizeForLot,
  resampleNearest,
  slotCoverage,
  slotPaint,
  slotsFromLot,
  type LotSlots,
} from "./ground";

const grass = { pattern: 8, color: [60, 110, 40] as [number, number, number] };
const road = { pattern: 4, color: [70, 70, 75] as [number, number, number] };

describe("lot ground model", () => {
  it("derives the mask size from the lot size at 0.75 m/px", () => {
    expect(maskSizeForLot([48, 96])).toEqual({ width: 64, height: 128 });
    expect(maskSizeForLot([0.1, 99999])).toEqual({ width: 1, height: 4096 });
  });

  it("paints one channel per material", () => {
    expect(slotPaint(0)).toEqual([255, 0, 0, 0]);
    expect(slotPaint(3)).toEqual([0, 0, 0, 255]);
  });

  it("round-trips materials through LotColor and keeps unused slots", () => {
    const slots: LotSlots = [grass, null, road, null];
    const previous = [[1, 2, 3, 4], [5, 6, 7, 9], [8, 8, 8, 8], [10, 11, 12, 13]];
    const colors = lotColorsFromSlots(slots, previous);
    expect(colors).toEqual([[60, 110, 40, 8], [5, 6, 7, 9], [70, 70, 75, 4], [10, 11, 12, 13]]);
    expect(slotsFromLot(colors, [true, false, true, false])).toEqual(slots);
    expect(firstFreeSlot(slots)).toBe(1);
  });

  it("shows the engine winner and leaves uncovered pixels transparent", () => {
    // Pixel 0: R and B both set -> B wins (A > B > G > R). Pixel 1: uncovered.
    const pixels = new Uint8ClampedArray([255, 0, 255, 0, 0, 0, 0, 0]);
    const display = groundDisplayPixels(pixels, [grass, null, road, null]);
    expect([...display]).toEqual([70, 70, 75, 255, 0, 0, 0, 0]);
    expect(slotCoverage(pixels)).toEqual([0, 0, 1, 0]);
  });

  it("tiles atlas cells onto painted pixels when cells are provided", () => {
    const pixels = new Uint8ClampedArray([255, 0, 0, 0]);
    const cell = {
      width: 1,
      height: 1,
      data: new Uint8ClampedArray([200, 100, 50, 255]),
    } as ImageData;
    const display = groundDisplayPixels(
      pixels,
      [{ pattern: 1, color: [128, 128, 128] }, null, null, null],
      [],
      [],
      { width: 1, height: 1, tilePx: [1, 1], cells: [cell, null, null, null] },
    );
    expect([...display]).toEqual([200, 100, 50, 255]);
  });

  it("clears only the removed material channel", () => {
    const pixels = new Uint8ClampedArray([0, 0, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0]);
    expect(clearSlotPixels(pixels, 3, 0)).toEqual({ x: 1, y: 0, w: 1, h: 1 });
    expect([...pixels]).toEqual([0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0]);
    expect(clearSlotPixels(pixels, 3, 0)).toBeNull();
  });

  it("resamples with nearest neighbour so channels stay one-hot", () => {
    const pixels = new Uint8ClampedArray([255, 0, 0, 0, 0, 0, 255, 0]);
    expect([...resampleNearest(pixels, 2, 1, 4, 1)]).toEqual([
      255, 0, 0, 0, 255, 0, 0, 0, 0, 0, 255, 0, 0, 0, 255, 0,
    ]);
  });
});
