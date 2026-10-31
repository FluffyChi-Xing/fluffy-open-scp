import type { EffectEmitterData, SwarmParticle } from "@/api/tauri";

/**
 * Swarm 局部单位与 lot 场景单位（米）的换算。
 *
 * 真实样本（如 00B1B104:43E1C000:DBAC1CFC）的粒子 size 只有 0.5–30、
 * block offset 1–6；这些值与建筑 bbox（约 76×49×37 m）同量级，说明效果资产
 * 直接以米书写。早期把 200–800 的烟雾当成厘米的假设会让普通效果缩小 100 倍
 * （实测几乎不可见），因此默认比例是 1。
 *
 * 对拍时仍可用 options.unitScale 或 localStorage 覆盖。
 */
export const SWARM_UNIT_SCALE = 1;

/**
 * 对拍用校准入口：在不改代码的前提下切换原始/厘米比例。
 *
 *   localStorage.setItem("openscpSwarmUnitScale", "0.01") // 厘米 → 米（对照）
 *   localStorage.setItem("openscpSwarmUnitScale", "1")    // 原始单位（默认）
 */
export function resolveSwarmUnitScale(): number {
  try {
    if (typeof localStorage !== "undefined") {
      const raw = localStorage.getItem("openscpSwarmUnitScale");
      if (raw) {
        const value = Number(raw);
        if (Number.isFinite(value) && value > 0) return value;
      }
    }
  } catch {
    // localStorage 在测试/SSR 环境可能不可用，回退默认值。
  }
  return SWARM_UNIT_SCALE;
}

function scaleTuple3(
  value: [number, number, number],
  scale: number,
): [number, number, number] {
  return [value[0] * scale, value[1] * scale, value[2] * scale];
}

function scaleTuple2(value: [number, number], scale: number): [number, number] {
  return [value[0] * scale, value[1] * scale];
}

export function toSceneParticle(
  particle: SwarmParticle,
  unitScale = SWARM_UNIT_SCALE,
): SwarmParticle {
  if (unitScale === 1) return particle;
  return {
    ...particle,
    emitSpeed: scaleTuple2(particle.emitSpeed, unitScale),
    emitVolumeMin: scaleTuple3(particle.emitVolumeMin, unitScale),
    emitVolumeMax: scaleTuple3(particle.emitVolumeMax, unitScale),
    size: particle.size.map((value) => value * unitScale),
    directionForce: scaleTuple3(particle.directionForce, unitScale),
    windStrength: particle.windStrength * unitScale,
    gravityStrength: particle.gravityStrength * unitScale,
  };
}

export function toSceneEmitter(
  emitter: EffectEmitterData,
  unitScale = SWARM_UNIT_SCALE,
): EffectEmitterData {
  return {
    ...emitter,
    transform: {
      ...emitter.transform,
      offset: scaleTuple3(emitter.transform.offset, unitScale),
    },
    particle: toSceneParticle(emitter.particle, unitScale),
    emission: emitter.emission ? toSceneParticle(emitter.emission, unitScale) : null,
  };
}
