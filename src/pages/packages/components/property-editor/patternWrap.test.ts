/**
 * 图案层 wrap 采样的经验回归测试：合成已知列梯度的法线格，1:1 密度合成后
 * 逐列扫描亮度剖面——任何列相对解析值的异常 = wrap 断裂/NaN 黑线。
 * （2026-09-29 用户对拍"图案重复周期处竖向切割痕"的定位测试。）
 */
import { describe, expect, it } from "vitest";
import {
  composeGroundPixels,
  type GroundComposeInput,
  type Pixels,
} from "./groundCompose";

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

describe("图案层 wrap 采样（切割痕定位）", () => {
  it("1:1 密度下逐列亮度 = 解析剖面，重复边界无断裂/黑线", () => {
    const mask = pixels(64, 64, () => [255, 0, 0, 0]);
    // cell 0（图集左上 256²）：nx 列梯度 −1→+1（byte 0→255）、ny = 128；
    // 其余格平坦。shade = 1 + 1.4·(nx·−0.5 + 0) = 1 − 0.7nx，clamp[0.55,1.45]。
    const atlas = pixels(1024, 1024, (x, y) =>
      x < 256 && y < 256 ? [x, 128, 255, 255] : [128, 128, 255, 255],
    );
    const base = pixels(4, 4, () => [200, 210, 220, 255]);
    const input: GroundComposeInput = {
      mask,
      rawMask: pixels(64, 64, () => [255, 0, 0, 0]),
      baseTile: base,
      lotColors: [[200, 180, 160, 0], [0, 0, 0, 1], [0, 0, 0, 2], [0, 0, 0, 3]],
      lotBorderColors: null,
      lotBorderPatternIndices: null,
      lotBorderWidths: [0, 0, 0, 0],
      normalAtlas: atlas,
      tilesX: 6,
      tilesY: 6,
      outSize: { width: 1536, height: 1536 },
    };
    const out = composeGroundPixels(input);

    // 解析列剖面：输出像素中心 u → frac((u−0.5)·6)·256 − 0.5 → nx 线性插值；
    // 明暗在线性空间相乘后回 sRGB（与实现同式，gamma 不可省）。
    const decode = (byte: number): number => {
      const srgb = byte / 255;
      return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
    };
    const encode = (linear: number): number => {
      const srgb =
        linear <= 0.0031308 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055;
      return Math.round(srgb * 255);
    };
    const expectColumn = (x: number): number => {
      const u = (x + 0.5) / out.width;
      const fx = ((u - 0.5) * 6 - Math.floor((u - 0.5) * 6)) * 256 - 0.5;
      const x0 = Math.floor(fx);
      const tx = fx - x0;
      const wx0 = x0 < 0 ? x0 + 256 : x0;
      const wx1 = x0 + 1 >= 256 ? x0 + 1 - 256 : x0 + 1;
      const nx = (wx0 * (1 - tx) + wx1 * tx) / 127.5 - 1;
      const shade = Math.min(1.45, Math.max(0.55, 1 - 0.7 * nx));
      return encode(decode(200) * shade);
    };
    const columnMean = (x: number): number => {
      let sum = 0;
      for (let y = 0; y < out.height; y += 1) sum += out.albedo[(y * out.width + x) * 4];
      return sum / out.height;
    };
    let worst = 0;
    let worstX = -1;
    for (let x = 0; x < out.width; x += 1) {
      const deviation = Math.abs(columnMean(x) - expectColumn(x));
      if (deviation > worst) {
        worst = deviation;
        worstX = x;
      }
    }
    expect(worst).toBeLessThan(1.5);
    expect(worstX).toBeGreaterThanOrEqual(0);
  });

  it("相位 wrap 处（fx<0 / fx≥W）不产生 NaN 黑像素", () => {
    const atlas = pixels(1024, 1024, (x, y) =>
      x < 256 && y < 256 ? [255, 255, 255, 255] : [128, 128, 255, 255],
    );
    const out = composeGroundPixels({
      mask: pixels(8, 8, () => [255, 0, 0, 0]),
      rawMask: pixels(8, 8, () => [255, 0, 0, 0]),
      baseTile: pixels(4, 4, () => [200, 210, 220, 255]),
      lotColors: [[200, 180, 160, 0], [0, 0, 0, 1], [0, 0, 0, 2], [0, 0, 0, 3]],
      lotBorderColors: null,
      lotBorderPatternIndices: null,
      lotBorderWidths: [0, 0, 0, 0],
      normalAtlas: atlas,
      tilesX: 6,
      tilesY: 6,
      outSize: { width: 1536, height: 1536 },
    });
    // nx=+1、ny=+1 → shade = 1 − 1.4 = clamp 0.55；绝不允许 NaN→0（黑）。
    let min = 255;
    for (let i = 0; i < out.width * out.height; i += 1) {
      const value = out.albedo[i * 4];
      if (value < min) min = value;
    }
    expect(min).toBeGreaterThan(0);
  }, 15000);
});
