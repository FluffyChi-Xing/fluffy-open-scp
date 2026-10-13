import type { SwarmParticle } from "@/api/tauri";

export interface ParticleState {
  position: [number, number, number];
  velocity: [number, number, number];
  age: number;
  life: number;
}

function curve(values: number[], phase: number, fallback: number): number {
  if (!values.length) return fallback;
  if (values.length === 1) return values[0];
  const at = Math.max(0, Math.min(1, phase)) * (values.length - 1);
  const lo = Math.floor(at);
  const hi = Math.min(values.length - 1, lo + 1);
  return values[lo] + (values[hi] - values[lo]) * (at - lo);
}

export function sampleScalar(values: number[], phase: number, fallback = 1): number {
  return curve(values, phase, fallback);
}

export function sampleColor(values: [number, number, number][], phase: number): [number, number, number] {
  if (!values.length) return [1, 1, 1];
  const at = Math.max(0, Math.min(1, phase)) * (values.length - 1);
  const lo = values[Math.floor(at)];
  const hi = values[Math.min(values.length - 1, Math.floor(at) + 1)];
  const mix = at - Math.floor(at);
  return [
    lo[0] + (hi[0] - lo[0]) * mix,
    lo[1] + (hi[1] - lo[1]) * mix,
    lo[2] + (hi[2] - lo[2]) * mix,
  ];
}

export class ParticleSolver {
  readonly particles: ParticleState[] = [];
  private seed: number;
  private elapsed = 0;
  private fraction = 0;
  private remainder = 0;

  constructor(
    readonly appearance: SwarmParticle,
    readonly source: SwarmParticle,
    seed: number,
    readonly capacity: number,
  ) {
    this.seed = seed >>> 0;
  }

  private random(): number {
    this.seed = (this.seed + 0x6d2b79f5) >>> 0;
    let value = Math.imul(this.seed ^ (this.seed >>> 15), 1 | this.seed);
    value ^= value + Math.imul(value ^ (value >>> 7), 61 | value);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  }

  private range(a: number, b: number): number {
    return a + (b - a) * this.random();
  }

  advance(delta: number): void {
    this.remainder = Math.min(this.remainder + Math.max(0, delta), 0.15);
    while (this.remainder >= 1 / 60) {
      this.step(1 / 60);
      this.remainder -= 1 / 60;
    }
  }

  private step(dt: number): void {
    this.elapsed += dt;
    const source = this.source;
    const phase = source.rateLoop > 0 ? (this.elapsed % source.rateLoop) / source.rateLoop : 0;
    const rate = Math.min(500, Math.max(0, sampleScalar(source.rate, phase, 0)));
    this.fraction += rate * dt;
    let births = Math.min(Math.floor(this.fraction), this.capacity - this.particles.length);
    this.fraction -= Math.floor(this.fraction);
    while (births-- > 0) {
      const direction: [number, number, number] = [0, 1, 2].map((axis) =>
        this.range(source.emitDirectionMin[axis], source.emitDirectionMax[axis]),
      ) as [number, number, number];
      const length = Math.hypot(...direction) || 1;
      const speed = this.range(...source.emitSpeed);
      this.particles.push({
        position: [0, 1, 2].map((axis) =>
          this.range(source.emitVolumeMin[axis], source.emitVolumeMax[axis]),
        ) as [number, number, number],
        velocity: direction.map((value) => value / length * speed) as [number, number, number],
        age: 0,
        life: Math.max(0.05, this.range(...this.appearance.life)),
      });
    }
    for (let i = this.particles.length - 1; i >= 0; i--) {
      const particle = this.particles[i];
      particle.age += dt;
      if (particle.age >= particle.life) {
        this.particles.splice(i, 1);
        continue;
      }
      const damping = Math.exp(-Math.max(0, source.drag) * dt);
      // Swarm exposes wind as a scalar rather than a vector. A stable, slow
      // lateral field gives material and smoke particles visible motion while
      // keeping previews deterministic across renders.
      const wind = source.windStrength * Math.sin(this.elapsed * 0.7 + i * 0.31);
      for (let axis = 0; axis < 3; axis++) {
        const force = source.directionForce[axis]
          + (axis === 0 ? wind : 0)
          + (axis === 2 ? -source.gravityStrength : 0);
        particle.velocity[axis] = (particle.velocity[axis] + force * dt) * damping;
        particle.position[axis] += particle.velocity[axis] * dt;
      }
    }
  }
}
