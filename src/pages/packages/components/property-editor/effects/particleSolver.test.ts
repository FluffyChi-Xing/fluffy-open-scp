import { describe, expect, it } from "vitest";
import type { SwarmParticle } from "@/api/tauri";
import { ParticleSolver, sampleScalar } from "./particleSolver";

const particle: SwarmParticle = {
  typeCode: 1, index: 0, flags: 0,
  life: [1, 1],
  emitDirectionMin: [0, 0, 1], emitDirectionMax: [0, 0, 1],
  emitSpeed: [2, 2],
  emitVolumeMin: [0, 0, 0], emitVolumeMax: [0, 0, 0],
  rate: [120], rateLoop: 0,
  size: [1, 2], aspect: [1], rotation: [0], alpha: [1, 0], color: [[1, 1, 1]],
  texture: { group: 0, instance: 0, drawMode: 4 },
  tileCount: [1, 1], frameStart: 0, frameCount: 1, frameRandom: 0, frameSpeed: 0, frameSpeedByLife: false,
  directionForce: [0, 0, 0], windStrength: 0, gravityStrength: 0, drag: 0,
};

describe("ParticleSolver", () => {
  it("uses a fixed step and reproducible seed within its particle budget", () => {
    const first = new ParticleSolver(particle, particle, 42, 8);
    const second = new ParticleSolver(particle, particle, 42, 8);
    first.advance(0.1);
    second.advance(0.05);
    second.advance(0.05);
    expect(first.particles).toEqual(second.particles);
    expect(first.particles).toHaveLength(8);
    first.advance(1.1);
    expect(first.particles.length).toBeLessThanOrEqual(8);
  });

  it("interpolates authored curves", () => {
    expect(sampleScalar([1, 3, 5], 0.25)).toBe(2);
    expect(sampleScalar([], 0.5, 0)).toBe(0);
  });

  it("applies the decoded wind scalar as deterministic lateral motion", () => {
    const calm = new ParticleSolver({ ...particle, windStrength: 0 }, particle, 7, 8);
    const windy = new ParticleSolver({ ...particle, windStrength: 2 }, { ...particle, windStrength: 2 }, 7, 8);
    calm.advance(0.5);
    windy.advance(0.5);
    expect(windy.particles).toHaveLength(calm.particles.length);
    expect(windy.particles.some((item, index) => item.position[0] !== calm.particles[index].position[0])).toBe(true);
  });
});
