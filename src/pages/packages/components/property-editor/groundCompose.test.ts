import { describe, expect, it } from "vitest";
import {
  composeGroundPixels,
  groundOutputSize,
  type GroundComposeInput,
  type Pixels,
} from "./groundCompose";

/** 构造纯色/函数填充的 RGBA Pixels。 */
function pixels(
  width: number,
  height: number,
  fill?: (x: number, y: number) => [number, number, number, number],
): Pixels {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const [r, g, b, a] = fill?.(x, y) ?? [0, 0, 0, 0];
      const at = (y * width + x) * 4;
      data[at] = r;
      data[at + 1] = g;
      data[at + 2] = b;
      data[at + 3] = a;
    }
  }
  return { data, width, height };
}

/** 4×4 格图集：格 i 的唯一色 = [i, 64, 255]（便于断言选格正确性）。 */
function atlas16(): Pixels {
  return pixels(4, 4, (x, y) => [y * 4 + x, 64, 255, 255]);
}

function input(overrides: Partial<GroundComposeInput>): GroundComposeInput {
  return {
    mask: pixels(1, 1, () => [0, 0, 0, 0]),
    rawMask: pixels(1, 1, () => [0, 0, 0, 0]),
    baseTile: pixels(1, 1, () => [200, 210, 220, 255]),
    lotColors: [
      [10, 20, 30, 0],
      [40, 50, 60, 1],
      [70, 80, 90, 2],
      [100, 110, 120, 3],
    ],
    lotBorderColors: [
      [11, 21, 31],
      [41, 51, 61],
      [71, 81, 91],
      [101, 111, 121],
    ],
    lotBorderPatternIndices: [4, 5, 6, 7],
    lotBorderWidths: [0, 0, 0, 0],
    normalAtlas: null,
    tilesX: 1,
    tilesY: 1,
    ...overrides,
  };
}

describe("groundCompose 引擎语义（generic_lot 直译）", () => {
  it("samples the original atlas V axis along negative world Y", () => {
    const out = composeGroundPixels(input({
      baseTile: pixels(1, 4, (_, y) => [40 * (y + 1), 0, 0, 255]),
      normalAtlas: pixels(4, 16, (_, y) => [128, 40 * (y % 4 + 1), 255, 255]),
      baseTileIndex: 0,
      outSize: { width: 4, height: 4 },
    }));
    // Output row centers have world Y = [-.375,-.125,.125,.375].
    // frac(-Y) samples source rows [1,0,3,2], not [2,3,0,1].
    expect([0, 1, 2, 3].map(y => out.albedo[y * 16])).toEqual([80, 40, 160, 120]);
    expect(out.normal[1]).toBeLessThan(128);
    expect(out.normal[2 * 16 + 1]).toBeGreaterThan(128);
  });

  it("keeps albedo independent of the normal direction", () => {
    const make = (red: number) => composeGroundPixels(input({
      rawMask: pixels(1, 1, () => [255, 0, 0, 0]),
      normalAtlas: pixels(4, 4, () => [red, 128, 255, 255]),
    }));
    const left = make(64), right = make(192);
    expect(left.albedo).toEqual(right.albedo);
    expect(left.normal[0]).toBeLessThan(128);
    expect(right.normal[0]).toBeGreaterThan(128);
  });

  it("authored height edges produce opposing normals on either side of a raised strip", () => {
    const out = composeGroundPixels(input({
      mask: pixels(16, 4),
      rawMask: pixels(16, 4, x => [255, x >= 5 && x <= 10 ? 255 : 0, 0, 0]),
      lotColorHeights: [0, 3, 0, 0],
      outSize: { width: 256, height: 64 },
    }));
    const row = Array.from({length: out.width}, (_, x) => out.normal[(32*out.width+x)*4]);
    expect(Math.min(...row.slice(60, 110))).toBeLessThan(100);
    expect(Math.max(...row.slice(150, 200))).toBeGreaterThan(155);
    expect(row[128]).toBe(128);
  });

  it("覆盖区 = 胜者通道平色直出（不采样漫反射、不乘 tint）", () => {
    // R+G 同时过阈 → 瀑布 A>B>G>R → G（通道 1）胜出。
    const out = composeGroundPixels(
      input({
        mask: pixels(2, 1),
        rawMask: pixels(2, 1, (x) => (x === 0 ? [255, 255, 0, 0] : [0, 0, 0, 0])),
      }),
    );
    // 输出 8×4（4× 超采样）：左半 = LC2 平色，逐字节相等（tile×tint 已证伪）。
    for (let x = 0; x < 4; x += 1) {
      const at = x * 4;
      expect([out.albedo[at], out.albedo[at + 1], out.albedo[at + 2]]).toEqual([
        40, 50, 60,
      ]);
      expect(out.albedo[at + 3]).toBe(255);
    }
  });

  it("优先级瀑布：A 边框带 > A 主色 > 低优先通道", () => {
    const lotBorderColors = input({}).lotBorderColors;
    // 像素 0：A=0.549、bw=0.1 → A 边框带（0.4 < 0.549 ≤ 0.6）；
    // 像素 1：A=0.8 → A 主色。
    const out = composeGroundPixels(
      input({
        mask: pixels(2, 1),
        rawMask: pixels(2, 1, (x) => (x === 0 ? [255, 0, 0, 140] : [0, 0, 0, 204])),
        lotBorderWidths: [0.1, 0.1, 0.1, 0.1],
      }),
    );
    const left = [out.albedo[0], out.albedo[1], out.albedo[2]];
    expect(left).toEqual(lotBorderColors![3]);
    // 右半（mask 像素 1，输出列 4-7）→ A 主色平色。
    const right = [out.albedo[16], out.albedo[17], out.albedo[18]];
    expect(right).toEqual([100, 110, 120]);
  });

  it("未覆盖区底图格按周期平铺（与图案层同密度，消大 lot 马赛克）", () => {
    // 纯色 1×1 底图格 + tiles 2×2：无 lotSize → outSize 回退 mask×4 = 8×8，
    // 平铺周期 = 4 输出像素——任意相隔一个周期的列/行均亮必须一致。
    const out = composeGroundPixels(
      input({
        mask: pixels(2, 2),
        rawMask: pixels(2, 2, () => [0, 0, 0, 0]),
        baseTile: pixels(1, 1, () => [34, 139, 34, 255]),
        tilesX: 2,
        tilesY: 2,
      }),
    );
    const colMean = (x: number): number => {
      let sum = 0;
      for (let y = 0; y < out.height; y += 1) sum += out.albedo[(y * out.width + x) * 4];
      return sum / out.height;
    };
    for (let x = 0; x < out.width / 2; x += 1) {
      expect(colMean(x)).toBe(colMean(x + out.width / 2));
    }
    // 纯色 1×1 格双线性无插值效应 → 逐字节直出。
    expect(colMean(0)).toBe(34);
  });

  it("底图平铺双线性：纹素间平滑过渡（非最近邻硬边）", () => {
    // base 2×1 [red, blue]，tilesX=2、outSize 8 → 每纹素 2 输出 px：
    // x=1 → u=0.1875 → fx=0.25 → red:0.75 + blue:0.25 的双线性混合。
    const out = composeGroundPixels(
      input({
        mask: pixels(2, 2),
        rawMask: pixels(2, 2, () => [0, 0, 0, 0]),
        baseTile: pixels(2, 1, (x) => (x === 0 ? [255, 0, 0, 255] : [0, 0, 255, 255])),
        tilesX: 2,
        tilesY: 1,
      }),
    );
    const at = 1 * 4; // 像素 (1,0)
    expect(out.albedo[at]).toBe(191);
    expect(out.albedo[at + 1]).toBe(0);
    expect(out.albedo[at + 2]).toBe(64);
  });

  it("主区颜色独立于法线（主区格号 = LotColor.A）", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [255, 0, 0, 0]),
        normalAtlas: atlas16(),
      }),
    );
    // R 通道胜出，LotColor.A=0 → 格 0（图集 px [0,64,255]）：
    // nx<0，仅改变单独输出的法线；反照率保持原色。
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual(
      [10, 20, 30],
    );
    expect(out.normal.length).toBe(out.albedo.length);
    expect(out.normal[0]).toBeLessThan(128);
    expect(out.albedo.length).toBe(out.width * out.height * 4);
  });

  it("倾斜法线不改变固有颜色", () => {
    // 自建图集：格 0 px = [255,128,255]，倾斜法线不能污染反照率。
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [255, 0, 0, 0]),
        normalAtlas: pixels(4, 4, (x, y) =>
          x === 0 && y === 0 ? [255, 128, 255, 255] : [128, 128, 255, 255],
        ),
      }),
    );
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual(
      [10, 20, 30],
    );
  });

  it("边框带颜色和法线分别输出", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [0, 0, 0, 140]),
        lotBorderWidths: [0, 0, 0, 0.1],
        normalAtlas: atlas16(),
      }),
    );
    // A 边框带反照率来自 LotBorderColor4，图案格 7 单独写入法线。
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual(
      [101, 111, 121],
    );
  });

  it("无法线图集时覆盖区保留固有色", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [255, 0, 0, 0]),
        normalAtlas: null,
      }),
    );
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual([10, 20, 30]);
  });

  it("未覆盖区反照率仍来自底图格", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [0, 0, 0, 0]),
        normalAtlas: atlas16(),
      }),
    );
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual([200, 210, 220]);
  });
});

describe("groundOutputSize（图案原生密度 = 探针 hires 口径）", () => {
  it("72m lot × 周期 8 × 格 256 → 2304²（32px/m，恰为探针 ppm=32）", () => {
    expect(groundOutputSize(128, 128, 9, 9, 256)).toEqual({
      width: 2304,
      height: 2304,
    });
  });

  it("192×96m → 6144×3072 → 4096×2048（探针同款上限，等比缩保长宽比）", () => {
    expect(groundOutputSize(256, 128, 24, 12, 256)).toEqual({
      width: 4096,
      height: 2048,
    });
  });

  it("无图案格 / 密度低于 mask×2 → 回退 mask×4（上限 1024）", () => {
    expect(groundOutputSize(128, 128, 1, 1, 0)).toEqual({ width: 512, height: 512 });
    expect(groundOutputSize(128, 128, 9, 9, 1)).toEqual({ width: 512, height: 512 });
  });
});
