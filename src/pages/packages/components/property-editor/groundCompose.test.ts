import { describe, expect, it } from "vitest";
import {
  composeGroundPixels,
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

  it("未覆盖区 = 底图格整格拉伸 + U 轴镜像（§5b）", () => {
    // 底图格 2×1 [red, blue]：左半列（u≈0.25）镜像后采 px1（blue），
    // 右半列（u≈0.75）采 px0（red）——列序与 mask 相反。
    const out = composeGroundPixels(
      input({
        mask: pixels(2, 1),
        rawMask: pixels(2, 1, () => [0, 0, 0, 0]),
        baseTile: pixels(2, 1, (x) => (x === 0 ? [255, 0, 0, 255] : [0, 0, 255, 255])),
      }),
    );
    const left = [out.albedo[0], out.albedo[1], out.albedo[2]];
    const right = [
      out.albedo[(out.width - 1) * 4],
      out.albedo[(out.width - 1) * 4 + 1],
      out.albedo[(out.width - 1) * 4 + 2],
    ];
    expect(left).toEqual([0, 0, 255]);
    expect(right).toEqual([255, 0, 0]);
  });

  it("图案质感走法线图集：主区格号 = LotColor.A", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [255, 0, 0, 0]),
        normalAtlas: atlas16(),
      }),
    );
    expect(out.normal).not.toBeNull();
    // R 通道胜出，LotColor.A=0 → 格 0（图集 px (0,0) = [0,64,255]）。
    const at = 0;
    expect([out.normal![at], out.normal![at + 1], out.normal![at + 2]]).toEqual([
      0, 64, 255,
    ]);
  });

  it("边框带图案格号 = LotBorderColor.A", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [0, 0, 0, 140]),
        lotBorderWidths: [0, 0, 0, 0.1],
        normalAtlas: atlas16(),
      }),
    );
    // A 边框带 → 图案格 borderIndices[3]=7 → 图集 px (3,1) = [7,64,255]。
    expect([out.normal![0], out.normal![1], out.normal![2]]).toEqual([7, 64, 255]);
    // 反照率 = LotBorderColor4 平色。
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual([101, 111, 121]);
  });

  it("无胜者像素法线平坦（引擎 overlayMask=0 无图案光照）", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [0, 0, 0, 0]),
        normalAtlas: atlas16(),
      }),
    );
    expect([out.normal![0], out.normal![1], out.normal![2]]).toEqual([128, 128, 255]);
  });
});
