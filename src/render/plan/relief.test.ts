import { describe, expect, it } from "vitest";
import { RELIEF_CONSTANTS, planRelief, reliefHasHeight } from "./relief";
import { RELIEF_GLSL } from "../exec/three/reliefShader";

describe("relief 计划", () => {
  it("常量与发行版 building4 片段一致（16000/0.16/23/8+2）", () => {
    expect(RELIEF_CONSTANTS).toEqual({
      cullDistanceSq: 16000,
      depth: 0.16,
      flatLevelInt: 23,
      maxConeRatio: 0.5,
      coneSteps: 8,
      binarySteps: 2,
    });
  });

  it("只有开关打开且材质有高度时才启用", () => {
    expect(planRelief({ toggle: true, hasHeight: true }).enabled).toBe(true);
    expect(planRelief({ toggle: true, hasHeight: false }).enabled).toBe(false);
    expect(planRelief({ toggle: false, hasHeight: true }).enabled).toBe(false);
    expect(planRelief({ toggle: true, hasHeight: true }).flatLevel).toBeCloseTo(23 / 255);
  });

  it("GLSL 使用发行版常量，不混入 authoring 变体（160000/0.1）", () => {
    expect(RELIEF_GLSL).toContain("16000.0");
    expect(RELIEF_GLSL).not.toContain("160000.0");
    expect(RELIEF_GLSL).toContain("0.160");
    expect(RELIEF_GLSL).not.toContain("0.100");
    expect(RELIEF_GLSL).toContain("kConeSteps = 8");
    expect(RELIEF_GLSL).toContain("kBinarySteps = 2");
    // 发行版没有“高度 <= 23 提前返回”的分支，只有 flatDepth 偏移。
    expect(RELIEF_GLSL).not.toContain("sourceControl");
  });
});

describe("reliefHasHeight", () => {
  const pixel = (r: number, g = 0, b = 0, a = 255) => [r, g, b, a];

  it("全平面图（<=23）判定为无高度", () => {
    const rgba = new Uint8Array(Array.from({ length: 100 }, () => pixel(10)).flat());
    expect(reliefHasHeight(rgba)).toBe(false);
  });

  it("存在超过 1% 的高度像素才判定为有高度", () => {
    const flat = Array.from({ length: 100 }, () => pixel(10)).flat();
    expect(reliefHasHeight(new Uint8Array(flat))).toBe(false);
    const withOne = [...flat];
    withOne.splice(0, 4, ...pixel(80));
    // 100 像素里 1 个 = 1%，阈值 >= 1% 成立
    expect(reliefHasHeight(new Uint8Array(withOne))).toBe(true);
  });
});
