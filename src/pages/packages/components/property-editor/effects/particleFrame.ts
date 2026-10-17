import type { SwarmParticle } from "@/api/tauri";
import type { ParticleState } from "./particleSolver";

export function particleAtlasFrame(
  particle: SwarmParticle,
  state: ParticleState,
): { current: number; next: number; mix: number } {
  const tiles = Math.max(1, particle.tileCount[0]) * Math.max(1, particle.tileCount[1]);
  const period = particle.frameCount || tiles;
  const position = Math.max(0, particle.frameSpeed) * (particle.frameSpeedByLife ? state.age / state.life : state.age);
  const base = particle.frameStart + state.frameOffset;
  const first = Math.floor(position);
  return {
    current: (base + first % period) % tiles,
    next: (base + (first + 1) % period) % tiles,
    mix: position % 1,
  };
}
