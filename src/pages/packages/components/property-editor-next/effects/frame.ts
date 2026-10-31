import type { SwarmParticle } from "@/api/tauri";
import type { ParticleState } from "./solver";

/**
 * Swarm 图集帧公式（对应 GenerateQuadInfo / StreamerSetup）：
 *
 *   frameValue = frameSpeed * age                    (frameSpeedByLife = false)
 *   frameValue = frameSpeed * age / life             (frameSpeedByLife = true)
 *   frameA = floor(frameValue), frameB = frameA + 1
 *   frameCount != 0 时先按 frameCount 取模，再加 frameStart
 *   UV = (frame % tilesU, frame / tilesU)，两帧之间按 fraction 插值
 */
export function particleAtlasFrame(
  particle: SwarmParticle,
  state: ParticleState,
): { current: number; next: number; mix: number } {
  const tiles = Math.max(1, particle.tileCount[0]) * Math.max(1, particle.tileCount[1]);
  const period = particle.frameCount || tiles;
  const position = Math.max(0, particle.frameSpeed)
    * (particle.frameSpeedByLife ? state.age / Math.max(state.life, 0.0001) : state.age);
  const base = particle.frameStart + state.frameOffset;
  const first = Math.floor(position);
  return {
    current: (base + (first % period)) % tiles,
    next: (base + ((first + 1) % period)) % tiles,
    mix: position % 1,
  };
}
