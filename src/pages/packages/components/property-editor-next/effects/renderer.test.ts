import { describe, expect, it } from "vitest";
import * as THREE from "three";
import type { EffectEmitterData, EffectUnit, SwarmParticle } from "@/api/tauri";
import { createParticleEffect } from "./renderer";

const particle: SwarmParticle = {
  typeCode: 1, index: 0, flags: 0,
  life: [1, 1],
  emitDirectionMin: [0, 0, 1], emitDirectionMax: [0, 0, 1],
  emitSpeed: [1, 1],
  emitVolumeMin: [0, 0, 0], emitVolumeMax: [0, 0, 0],
  rate: [10], rateLoop: 1,
  size: [100], aspect: [1], rotation: [0], alpha: [1], color: [[1, 1, 1]],
  texture: { group: 0, instance: 0, drawMode: 0 },
  tileCount: [1, 1], frameStart: 0, frameCount: 0, frameRandom: 0,
  frameSpeed: 0, frameSpeedByLife: false,
  directionForce: [0, 0, 0], windStrength: 0, gravityStrength: 0, drag: 0,
};

const emitter: EffectEmitterData = {
  transform: { scale: 1, rotation: [1, 0, 0, 0, 1, 0, 0, 0, 1], offset: [0, 0, 500] },
  particle,
  emission: null,
  texturePngBase64: null,
  fallback: false,
};

const unit = {
  kind: "effect",
  index: 0,
  transform: null,
  effectId: { typeId: 0, group: 0, instance: 1 },
  externalId: null,
  enabled: null,
  fields: [],
} as EffectUnit;

function quadMatrix(view: ReturnType<typeof createParticleEffect>): THREE.Mesh {
  return view.object.children[1] as THREE.Mesh;
}

describe("createParticleEffect (parallel)", () => {
  it("keeps the authored block offset in meters by default", () => {
    const view = createParticleEffect(THREE, unit, [emitter], 4);
    // Matrix4.set 的第 4 列即平移；column-major elements[14] 是 z。
    expect(quadMatrix(view).matrix.elements[14]).toBeCloseTo(500, 5);
    view.dispose();
  });

  it("can compare against the centimeter hypothesis", () => {
    const view = createParticleEffect(THREE, unit, [emitter], 4, { unitScale: 0.01 });
    expect(quadMatrix(view).matrix.elements[14]).toBeCloseTo(5, 5);
    view.dispose();
  });
});
