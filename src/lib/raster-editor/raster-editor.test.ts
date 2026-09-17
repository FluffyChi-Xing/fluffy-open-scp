import { describe, expect, it } from "vitest";
import { RasterDocument, unionRect } from "./document";
import { RasterHistory } from "./history";
import {
  drawRect,
  floodFill,
  stampBrush,
  strokePath,
  type Rgba,
} from "./tools";
import { base64ToRgba, rgbaToBase64 } from "./encoders";

const RED: Rgba = [255, 0, 0, 255];

describe("RasterDocument", () => {
  it("rejects non-positive dimensions and short buffers", () => {
    expect(() => new RasterDocument(0, 4)).toThrow();
    expect(() => new RasterDocument(2, 2, new Uint8ClampedArray(4))).toThrow();
  });

  it("copies and restores regions", () => {
    const doc = new RasterDocument(4, 2);
    doc.setPixel(1, 0, RED);
    const region = { x: 0, y: 0, w: 2, h: 2 };
    const before = doc.copyRegion(region);
    expect(before[4]).toBe(255); // 像素 (1,0) = RED

    doc.restoreRegion(region, new Uint8ClampedArray(region.w * region.h * 4));
    expect(doc.getPixel(1, 0)[0]).toBe(0);
    expect(before).not.toEqual(doc.copyRegion(region));
  });

  it("unions dirty rects", () => {
    expect(unionRect(null, { x: 2, y: 2, w: 1, h: 1 })).toEqual({
      x: 2,
      y: 2,
      w: 1,
      h: 1,
    });
    expect(
      unionRect({ x: 0, y: 0, w: 2, h: 2 }, { x: 3, y: 3, w: 1, h: 1 }),
    ).toEqual({ x: 0, y: 0, w: 4, h: 4 });
  });
});

describe("RasterHistory", () => {
  it("undoes and redoes region edits", () => {
    const doc = new RasterDocument(4, 4);
    const history = new RasterHistory(doc);
    const rect = { x: 0, y: 0, w: 2, h: 1 };
    const before = doc.copyRegion(rect);
    doc.setPixel(0, 0, RED);
    history.push({
      rect,
      before,
      after: doc.copyRegion(rect),
    });

    expect(history.canUndo).toBe(true);
    history.undo();
    expect(doc.getPixel(0, 0)[0]).toBe(0);
    history.redo();
    expect(doc.getPixel(0, 0)[0]).toBe(255);
    expect(history.canUndo).toBe(true);
  });

  it("clears the redo stack on a new edit", () => {
    const doc = new RasterDocument(4, 4);
    const history = new RasterHistory(doc);
    const rect = { x: 0, y: 0, w: 1, h: 1 };
    const empty = new Uint8ClampedArray(4);
    doc.setPixel(0, 0, RED);
    history.push({ rect, before: empty, after: doc.copyRegion(rect) });
    history.undo();
    doc.setPixel(1, 1, RED);
    history.push({
      rect,
      before: empty,
      after: doc.copyRegion(rect),
    });
    expect(history.canRedo).toBe(false);
  });
});

describe("tools", () => {
  it("stamps a brush inside bounds and reports dirty rect", () => {
    const doc = new RasterDocument(4, 4);
    const touched: { x: number; y: number }[] = [];
    stampBrush(doc, 3, 3, 1, RED, (x, y) => touched.push({ x, y }));
    expect(touched).toEqual([{ x: 3, y: 3 }]);
    expect(doc.getPixel(3, 3)).toEqual([255, 0, 0, 255]);
  });

  it("draws a line across the document", () => {
    const doc = new RasterDocument(8, 8);
    const dirty = strokePath(
      doc,
      [
        { x: 0, y: 0 },
        { x: 7, y: 7 },
      ],
      1,
      RED,
      () => {},
    );
    expect(dirty).not.toBeNull();
    // 对角线两端与中点都被上色
    expect(doc.getPixel(0, 0)[0]).toBe(255);
    expect(doc.getPixel(7, 7)[0]).toBe(255);
    expect(doc.getPixel(3, 3)[0]).toBe(255);
  });

  it("draws hollow rectangles", () => {
    const doc = new RasterDocument(6, 6);
    drawRect(doc, { x: 1, y: 1 }, { x: 4, y: 4 }, 1, RED, false, () => {});
    expect(doc.getPixel(1, 1)[0]).toBe(255);
    expect(doc.getPixel(4, 4)[0]).toBe(255);
    expect(doc.getPixel(2, 2)[3]).toBe(0);
  });

  it("flood fills a connected region", () => {
    const doc = new RasterDocument(4, 4);
    // 顶部两行同色（默认透明），左下角一个红点
    doc.setPixel(3, 3, RED);
    const dirty = floodFill(doc, 0, 0, [0, 255, 0, 255], () => {});
    // 除 (3,3) 红点外全部连通填充
    expect(dirty).toEqual({ x: 0, y: 0, w: 4, h: 4 });
    expect(doc.getPixel(2, 1)[1]).toBe(255);
    expect(doc.getPixel(3, 3)).toEqual([255, 0, 0, 255]);
  });
});

describe("base64", () => {
  it("round-trips rgba bytes", () => {
    const bytes = new Uint8ClampedArray([1, 2, 3, 4, 250, 251, 252, 253]);
    expect(base64ToRgba(rgbaToBase64(bytes))).toEqual(bytes);
  });
});
