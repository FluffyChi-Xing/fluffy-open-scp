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

  it("图案光照烘焙进反照率：平色 × 法线图集坡度明暗（主区格号 = LotColor.A）", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [255, 0, 0, 0]),
        normalAtlas: atlas16(),
      }),
    );
    // R 通道胜出，LotColor.A=0 → 格 0（图集 px [0,64,255]）：
    // nx=−1、ny≈−0.498 → shade = 1+1.4·(0.5+0.249) = 2.049 → clamp 1.45（向光上限）。
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual(
      bake([10, 20, 30], 1.45),
    );
    // 输出不再含 normal 通道（图案光照不走实时光照）。
    expect(out.albedo.length).toBe(out.width * out.height * 4);
  });

  it("图案光照双向：逆光面变暗（clamp 下限 0.55）", () => {
    // 自建图集：格 0 px = [255,128,255] → nx=+1、ny≈+0.004 →
    // shade = 1+1.4·(−0.5−0.002) ≈ 0.297 → clamp 0.55。
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
      bake([10, 20, 30], 0.55),
    );
  });

  it("边框带图案格号 = LotBorderColor.A，同款烘焙", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [0, 0, 0, 140]),
        lotBorderWidths: [0, 0, 0, 0.1],
        normalAtlas: atlas16(),
      }),
    );
    // A 边框带 → 反照率 = LotBorderColor4 × 图案格 borderIndices[3]=7 明暗
    //（格 7 px [7,64,255] → shade 同样触顶 1.45）。
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual(
      bake([101, 111, 121], 1.45),
    );
  });

  it("无法线图集时覆盖区 = 纯平色（shade=1，无明暗）", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [255, 0, 0, 0]),
        normalAtlas: null,
      }),
    );
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual([10, 20, 30]);
  });

  it("未覆盖区无图案光照：底图格逐字节直出", () => {
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [0, 0, 0, 0]),
        normalAtlas: atlas16(),
      }),
    );
    expect([out.albedo[0], out.albedo[1], out.albedo[2]]).toEqual([200, 210, 220]);
  });

  it("底图格最近邻放大：探针同款颗粒感（非双线性模糊）", () => {
    // 底图 2×1 [red, blue] 拉伸到 4×4 输出：x=1 列 u=0.375 → 1−u=0.625 →
    // floor(1.25)=1 → 纯 blue（双线性会得到 25% red + 75% blue 的混合）。
    const out = composeGroundPixels(
      input({
        mask: pixels(1, 1),
        rawMask: pixels(1, 1, () => [0, 0, 0, 0]),
        baseTile: pixels(2, 1, (x) => (x === 0 ? [255, 0, 0, 255] : [0, 0, 255, 255])),
      }),
    );
    const at = 1 * 4; // 像素 (1,0)
    expect([out.albedo[at], out.albedo[at + 1], out.albedo[at + 2]]).toEqual([
      0, 0, 255,
    ]);
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

/**
 * 烘焙公式（独立实现，= lot_composite tinted/shade_of 同款）：
 * 线性空间相乘后回 sRGB。
 */
function bake(flat: [number, number, number], shade: number): [number, number, number] {
  return [encode(decode(flat[0]) * shade), encode(decode(flat[1]) * shade), encode(decode(flat[2]) * shade)];
}
function decode(byte: number): number {
  const srgb = byte / 255;
  return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
}
function encode(linear: number): number {
  const srgb =
    linear <= 0.0031308 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055;
  return Math.round(srgb * 255);
}
