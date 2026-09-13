import { describe, expect, it } from "vitest";
import {
  averageSkyLuminance,
  createSunEnv,
  applySunEnv,
  dayFactor,
  skyRadiance,
  sunAltitude,
  type SunEnvRefs,
} from "./refinedRender";

async function makeEnv(timeOfDay: number): Promise<SunEnvRefs> {
  const THREE = await import("three");
  const env = createSunEnv(THREE);
  applySunEnv(env, timeOfDay, true);
  return env;
}

const lum = (c: [number, number, number]) =>
  0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];

describe("sun env time of day", () => {
  it("altitude/day factor follow the documented curve", () => {
    expect(sunAltitude(12)).toBeCloseTo(1, 6); // 正午
    expect(sunAltitude(0)).toBeCloseTo(-1, 6); // 子夜
    expect(dayFactor(12)).toBeCloseTo(1, 6);
    expect(dayFactor(0)).toBeCloseTo(0, 6);
  });

  it("sky luminance reference is recomputed per time of day", async () => {
    const noon = await makeEnv(12);
    const night = await makeEnv(0);
    expect(noon.skyLumRef.value).toBeGreaterThan(0.2);
    expect(night.skyLumRef.value).toBeLessThan(noon.skyLumRef.value);
    // 与独立重算一致（applySunEnv 必须刷新它，否则间接光整体漂移）
    expect(noon.skyLumRef.value).toBeCloseTo(averageSkyLuminance(noon), 6);
  });
});

/** 独立于实现的球面均匀采样（Fibonacci，N 与 refinedRender 内的 96 不同）。 */
function sphereSample(n: number): [number, number, number][] {
  return Array.from({ length: n }, (_, i) => {
    const z = 1 - (2 * (i + 0.5)) / n;
    const r = Math.sqrt(Math.max(0, 1 - z * z));
    const theta = Math.PI * (1 + Math.sqrt(5)) * i;
    return [r * Math.cos(theta), r * Math.sin(theta), z] as [number, number, number];
  });
}

describe("analytic sky radiance", () => {
  it("varies by direction (this is what a constant ambient could not do)", async () => {
    const env = await makeEnv(12);
    const up = lum(skyRadiance([0, 0, 1], env));
    const wall = lum(skyRadiance([1, 0, 0], env));
    const down = lum(skyRadiance([0, 0, -1], env));
    // 地平线附近最亮（大气散射）→ 天顶次之 → 朝下的地面反照最暗
    expect(wall).toBeGreaterThan(up);
    expect(up).toBeGreaterThan(down);
    // 峰值/谷值分离显著，否则「方向性」在视觉上等于没有
    expect(wall / down).toBeGreaterThan(2);
  });

  it("is brighter toward the sun than away from it", async () => {
    const env = await makeEnv(12);
    const s = env.sunDir.value;
    const toward = lum(skyRadiance([s.x, s.y, s.z], env));
    const away = lum(skyRadiance([-s.x, -s.y, -s.z], env));
    expect(toward).toBeGreaterThan(away);
  });

  it("direction factor averages to 1 over the sphere (no exposure drift)", async () => {
    // 着色器用 scSkyRadiance(法线) / uSkyLumRef 调制间接漫反射。均值必须为 1：
    // 否则「加方向性」会顺带改整体亮度，用户已调好的曝光会整体漂移。
    for (const t of [0, 6, 12, 18]) {
      const env = await makeEnv(t);
      const ref = averageSkyLuminance(env);
      expect(ref).toBeCloseTo(env.skyLumRef.value, 6);
      const dirs = sphereSample(257);
      const mean =
        dirs.reduce((sum, d) => sum + lum(skyRadiance(d, env)) / ref, 0) /
        dirs.length;
      expect(mean).toBeGreaterThan(0.97);
      expect(mean).toBeLessThan(1.03);
    }
  });
});
