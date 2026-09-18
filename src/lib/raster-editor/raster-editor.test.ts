import { describe, expect, it } from "vitest";
import { RasterDocument, clampRect, unionRect } from "./document";
import { RasterHistory } from "./history";
import {
  drawRect,
  floodFill,
  stampBrush,
  strokeParkingRow,
  strokePath,
  strokeQuadCurve,
  type Rgba,
} from "./tools";
import {
  base64ToRgba,
  rgbaToBase64,
  layerOfPixel,
} from "./encoders";

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

  it("flood fills anti-aliased regions within tolerance and stops at borders", () => {
    const doc = new RasterDocument(6, 4);
    // 渐变红区域（模拟抗锯齿）被深蓝边框包住
    for (let y = 0; y < 4; y += 1) {
      for (let x = 0; x < 6; x += 1) {
        const edge = x === 0 || x === 5 || y === 0 || y === 3;
        doc.setPixel(x, y, edge ? [20, 20, 160, 255] : [200 + x, 30, 40, 255]);
      }
    }
    const dirty = floodFill(doc, 2, 2, [0, 255, 0, 255], () => {}, 32);
    expect(dirty).toEqual({ x: 1, y: 1, w: 4, h: 2 });
    // 内部全部变绿，边框保持深蓝
    expect(doc.getPixel(2, 2)).toEqual([0, 255, 0, 255]);
    expect(doc.getPixel(4, 2)).toEqual([0, 255, 0, 255]);
    expect(doc.getPixel(0, 2)).toEqual([20, 20, 160, 255]);
    // 容差 0 时渐变区域只填中单色连通块
    const strict = new RasterDocument(6, 4);
    for (let y = 0; y < 4; y += 1) {
      for (let x = 0; x < 6; x += 1) {
        strict.setPixel(x, y, [200 + x, 30, 40, 255]);
      }
    }
    floodFill(strict, 2, 2, [0, 255, 0, 255], () => {}, 0);
    expect(strict.getPixel(2, 2)).toEqual([0, 255, 0, 255]);
    expect(strict.getPixel(3, 2)[0]).toBe(203); // 相邻列因色差未命中
  });

  it("stamps a parking row of evenly spaced perpendicular ticks", () => {
    const doc = new RasterDocument(24, 8);
    const dirty = strokeParkingRow(
      doc,
      { x: 0, y: 0 },
      { x: 18, y: 0 },
      1,
      RED,
      { length: 4, spacing: 6 },
      () => {},
    );
    // ticks at x = 0 / 6 / 12 / 18，各向下延伸 4px；脏区为宽松包围盒（含笔刷半径外扩）
    expect(dirty).not.toBeNull();
    expect(dirty!.x).toBeLessThanOrEqual(0);
    expect(dirty!.y).toBeLessThanOrEqual(0);
    expect(dirty!.x + dirty!.w).toBeGreaterThanOrEqual(19);
    expect(dirty!.y + dirty!.h).toBeGreaterThanOrEqual(4);
    for (const x of [0, 6, 12, 18]) {
      expect(doc.getPixel(x, 0)[0]).toBe(255);
      expect(doc.getPixel(x, 3)[0]).toBe(255);
    }
    for (const x of [3, 9, 15]) {
      expect(doc.getPixel(x, 0)[3]).toBe(0);
    }
    expect(doc.getPixel(1, 2)[3]).toBe(0);
  });

  it("draws a quadratic curve bowing away from the chord", () => {
    const doc = new RasterDocument(20, 12);
    strokeQuadCurve(
      doc,
      { x: 0, y: 0 },
      { x: 8, y: 8 },
      { x: 16, y: 0 },
      1,
      RED,
      () => {},
    );
    expect(doc.getPixel(0, 0)[0]).toBe(255);
    expect(doc.getPixel(16, 0)[0]).toBe(255);
    // 曲线中点 = 0.25*p0 + 0.5*p1 + 0.25*p2 = (8, 4)，弦上 (8,0) 不应被画
    expect(doc.getPixel(8, 4)[0]).toBe(255);
    expect(doc.getPixel(8, 0)[3]).toBe(0);
  });
});

describe("base64", () => {
  it("round-trips rgba bytes", () => {
    const bytes = new Uint8ClampedArray([1, 2, 3, 4, 250, 251, 252, 253]);
    expect(base64ToRgba(rgbaToBase64(bytes))).toEqual(bytes);
  });
});

describe("lot layers", () => {
  it("maps raw pixels to layers by A > R > G > B priority", () => {
    expect(layerOfPixel([0, 0, 0, 255])).toBe(0);
    expect(layerOfPixel([255, 0, 0, 200])).toBe(0); // A 优先于 R
    expect(layerOfPixel([255, 0, 0, 0])).toBe(1);
    expect(layerOfPixel([0, 255, 0, 0])).toBe(2);
    expect(layerOfPixel([0, 0, 255, 0])).toBe(3);
    expect(layerOfPixel([10, 10, 10, 10])).toBeNull();
  });
});

describe("clampRect", () => {
  it("intersects dirty rects with the document bounds", () => {
    const doc = new RasterDocument(8, 8);
    expect(clampRect({ x: -4, y: -2, w: 8, h: 8 }, doc)).toEqual({
      x: 0,
      y: 0,
      w: 4,
      h: 6,
    });
    expect(clampRect({ x: 9, y: 9, w: 4, h: 4 }, doc)).toBeNull();
  });
});
