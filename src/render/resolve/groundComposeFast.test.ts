import { describe, expect, it } from "vitest";
import {
  composeGroundPreview,
  composeGroundPreviewFast,
  type GroundComposeInput,
  type Pixels,
} from "./groundCompose";
function pixels(w: number, h: number, fill: (i: number) => number): Pixels {
  return {
    width: w,
    height: h,
    data: Uint8ClampedArray.from({ length: w * h * 4 }, (_, i) => fill(i)),
  };
}
describe("accelerated ground preview", () => {
  it("preserves every output byte across heights, soft boundaries, wrapping and degenerate masks", () => {
    let seed = 81;
    const random = () => {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      return seed >>> 24;
    };
    for (const width of [1, 7, 32])
      for (const border of [0, 0.0375, 0.48, 0.7])
        for (const kind of ["flat", "noise", "ramp"]) {
          const raw = pixels(width, 11, (i) =>
            kind === "flat"
              ? i % 4 === 0
                ? 255
                : 0
              : kind === "noise"
                ? random()
                : Math.floor(((i % (width * 4)) / (width * 4)) * 255),
          );
          const request: GroundComposeInput = {
            mask: pixels(width, 11, random),
            rawMask: raw,
            baseTile: pixels(5, 7, random),
            lotColors: [
              [20, 30, 40, 0],
              [40, 50, 60, 2],
              [100, 110, 120, 8],
              [200, 210, 220, 15],
            ],
            lotBorderColors: [
              [5, 6, 7],
              [8, 9, 10],
              [11, 12, 13],
              [14, 15, 16],
            ],
            lotBorderWidths: [border, border, border, border],
            lotBorderPatternIndices: [1, 2, 3, 4],
            lotColorHeights: [0, 1, -0.2, 0.5],
            lotBorderHeights: [0.5, 0.3, 0.8, -0.4],
            normalAtlas: pixels(20, 28, random),
            tilesX: 10.285846710205078,
            tilesY: 9.197941303253174,
            outSize: { width: 67, height: 51 },
            enhanceTintDetail: true,
          };
          const baseline = composeGroundPreview(request),
            result = composeGroundPreviewFast(request);
          expect(result.albedo).toEqual(baseline.albedo);
          expect(result.normal).toEqual(baseline.normal);
        }
  });
});
