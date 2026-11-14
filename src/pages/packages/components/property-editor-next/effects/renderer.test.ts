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

  it("uses an emitter-local XY plane for the pump's axial mode 5 particle", () => {
    const water = { ...emitter, particle: { ...particle, alignMode: 5 } };
    const view = createParticleEffect(THREE, unit, [water], 4);
    const material = quadMatrix(view).material as THREE.ShaderMaterial;
    expect(material.defines.SWARM_EMITTER_PLANE).toBe(1);
    expect(material.uniforms.uEmitterRight.value.toArray()).toEqual([1, 0]);
    view.dispose();
  });

  it("derives the mode 5 basis from the average emission direction", () => {
    const aligned: EffectEmitterData = {
      ...emitter,
      particle: {
        ...particle,
        alignMode: 5,
        emitDirectionMin: [1, 0, 0],
        emitDirectionMax: [1, 2, 0],
      },
    };
    const view = createParticleEffect(THREE, unit, [aligned, emitter], 4);
    const alignedMaterial = quadMatrix(view).material as THREE.ShaderMaterial;
    const [x, y] = alignedMaterial.uniforms.uEmitterRight.value.toArray();
    expect(x).toBeCloseTo(Math.SQRT1_2);
    expect(y).toBeCloseTo(-Math.SQRT1_2);
    const legacyMaterial = (view.object.children[2] as THREE.Mesh).material as THREE.ShaderMaterial;
    expect(legacyMaterial.defines.SWARM_EMITTER_PLANE).toBeUndefined();
    view.dispose();
  });
});
