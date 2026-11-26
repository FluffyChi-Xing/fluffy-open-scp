import { expect, it, vi } from "vitest";
import { RasterDocument } from "@/lib/raster-editor/document";
import { createLotPaintPreview } from "./paintPreview";

it("repaints only damaged pixels at their original material coordinates", () => {
  class TestPath {
    readonly runs: number[][] = [];
    rect(...rect: number[]) {
      this.runs.push(rect);
    }
  }
  vi.stubGlobal("Path2D", TestPath);
  try {
    const doc = new RasterDocument(5, 3);
    doc.setPixel(1, 1, [255, 0, 0, 0]);
    doc.setPixel(2, 1, [130, 0, 0, 0]);
    doc.setPixel(3, 1, [130, 0, 0, 255]);
    doc.setPixel(4, 1, [0, 0, 0, 255]);
    doc.setPixel(2, 2, [255, 0, 0, 0]);
    const ctx = { fill: vi.fn<(path: TestPath) => void>() };
    const preview = createLotPaintPreview(
      [null, null, null, null], [0.1, 0, 0, 0], [], [1, 1], [],
    );
    preview(ctx as unknown as CanvasRenderingContext2D, doc, { x: 2, y: 1, w: 2, h: 1 });
    const paths = ctx.fill.mock.calls.map(([path]) => path.runs);
    expect(paths).toEqual([[], [[2, 1, 1, 1]], [], [], [], [], [[3, 1, 1, 1]], []]);
  } finally {
    vi.unstubAllGlobals();
  }
});
