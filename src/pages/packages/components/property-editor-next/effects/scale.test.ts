import { describe, expect, it } from "vitest";
import type { EffectEmitterData, SwarmParticle } from "@/api/tauri";
import { SWARM_UNIT_SCALE, toSceneEmitter, toSceneParticle } from "./scale";

const particle: SwarmParticle = {
  typeCode: 1,
  index: 0,
  flags: 0,
  life: [15, 35],
  emitDirectionMin: [-1, -1, 0],
  emitDirectionMax: [1, 1, 2],
  emitSpeed: [0, 60],
  emitVolumeMin: [-1000, -1000, -100],
  emitVolumeMax: [1000, 1000, 100],
  rate: [27],
  rateLoop: 1,
  size: [200, 800],
  aspect: [1],
  rotation: [0],
  alpha: [0, 1],
  color: [[0.6, 0.4, 0.2]],
  texture: { group: 0, instance: 0xa9fd260c, drawMode: 15 },
  tileCount: [1, 16],
  frameStart: 0,
  frameCount: 0,
  frameRandom: 0,
  frameSpeed: 1,
  frameSpeedByLife: true,
  directionForce: [0, 0, 1],
  windStrength: 1.5,
  gravityStrength: -1.5,
  drag: 0.1,
};

describe("Swarm unit scale", () => {
  it("keeps authored meters by default", () => {
    expect(SWARM_UNIT_SCALE).toBe(1);
    const scaled = toSceneParticle(particle);
    expect(scaled.size).toEqual([200, 800]);
    expect(scaled.emitSpeed).toEqual([0, 60]);
    expect(scaled.emitVolumeMax).toEqual([1000, 1000, 100]);
    expect(scaled.directionForce).toEqual([0, 0, 1]);
    expect(scaled.windStrength).toBe(1.5);
  });

  it("can still express the centimeter hypothesis for comparison", () => {
    const scaled = toSceneParticle(particle, 0.01);
    expect(scaled.size).toEqual([2, 8]);
    expect(scaled.emitSpeed).toEqual([0, 0.6]);
    expect(scaled.emitVolumeMax).toEqual([10, 10, 1]);
    expect(scaled.directionForce).toEqual([0, 0, 0.01]);
    expect(scaled.windStrength).toBeCloseTo(0.015, 6);
    expect(scaled.gravityStrength).toBeCloseTo(-0.015, 6);
    expect(scaled.drag).toBe(0.1);
  });

  it("scales the block offset only when a conversion is requested", () => {
    const emitter: EffectEmitterData = {
      transform: { scale: 2, rotation: [1, 0, 0, 0, 1, 0, 0, 0, 1], offset: [0, 0, 500] },
      particle,
      emission: null,
      texturePngBase64: null,
      fallback: false,
    };
    expect(toSceneEmitter(emitter).transform.offset).toEqual([0, 0, 500]);
    expect(toSceneEmitter(emitter, 0.01).transform.offset).toEqual([0, 0, 5]);
    expect(toSceneEmitter(emitter, 0.01).transform.scale).toBe(2);
    expect(toSceneEmitter(emitter, 0.01).particle.size).toEqual([2, 8]);
  });
});
