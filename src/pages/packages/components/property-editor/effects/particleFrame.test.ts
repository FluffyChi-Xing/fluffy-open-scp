import { describe, expect, it } from "vitest";
import type { SwarmParticle } from "@/api/tauri";
import type { ParticleState } from "./particleSolver";
import { particleAtlasFrame } from "./particleFrame";

const appearance = {
  tileCount: [1, 16], frameStart: 0, frameCount: 0,
  frameSpeed: 2, frameSpeedByLife: false,
} as SwarmParticle;
const state: ParticleState = {
  position: [0, 0, 0], velocity: [0, 0, 0], age: 7.75, life: 10, frameOffset: 0,
};

describe("particleAtlasFrame", () => {
  it("wraps the whole atlas when frameCount is zero", () => {
    expect(particleAtlasFrame(appearance, state)).toEqual({ current: 15, next: 0, mix: 0.5 });
  });

  it("wraps a limited frame group while preserving its start", () => {
    expect(particleAtlasFrame({ ...appearance, frameStart: 4, frameCount: 4 }, { ...state, age: 1.75 }))
      .toEqual({ current: 7, next: 4, mix: 0.5 });
  });

  it("uses normalized life when requested by the effect", () => {
    expect(particleAtlasFrame({ ...appearance, frameSpeedByLife: true }, state))
      .toEqual({ current: 1, next: 2, mix: 0.55 });
  });
});
