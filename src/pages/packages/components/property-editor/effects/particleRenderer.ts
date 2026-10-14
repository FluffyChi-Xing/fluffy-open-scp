import type * as ThreeNamespace from "three";
import type { EffectEmitterData, EffectUnit } from "@/api/tauri";
import { unitMatrix } from "../unitGizmos";
import { ParticleSolver, sampleColor, sampleScalar } from "./particleSolver";
import { particleAtlasFrame } from "./particleFrame";

type Three = typeof ThreeNamespace;

export interface ParticleEffectView {
  object: ThreeNamespace.Group;
  tick(delta: number): void;
  dispose(): void;
}

function blockMatrix(THREE: Three, emitter: EffectEmitterData): ThreeNamespace.Matrix4 {
  const { rotation: r, offset: p, scale } = emitter.transform;
  if (r.length !== 9 || !r.every(Number.isFinite)) return new THREE.Matrix4();
  const matrix = new THREE.Matrix4().set(
    r[0], r[3], r[6], p[0],
    r[1], r[4], r[7], p[1],
    r[2], r[5], r[8], p[2],
    0, 0, 0, 1,
  );
  return matrix.multiply(new THREE.Matrix4().makeScale(scale, scale, scale));
}

export function createParticleEffect(
  THREE: Three,
  unit: EffectUnit,
  emitters: EffectEmitterData[],
  capacity: number,
): ParticleEffectView {
  const object = new THREE.Group();
  if (unit.transform) unitMatrix(THREE, unit.transform).decompose(object.position, object.quaternion, object.scale);
  const proxy = new THREE.Mesh(
    new THREE.SphereGeometry(0.6, 8, 6),
    new THREE.MeshBasicMaterial({ transparent: true, opacity: 0, colorWrite: false, depthWrite: false }),
  );
  proxy.userData.pickOnly = true;
  object.add(proxy);
  const views = emitters.map((emitter, index) => {
    const count = Math.max(1, Math.floor(capacity / emitters.length));
    const geometry = new THREE.BufferGeometry();
    const positions = new Float32Array(count * 4 * 3);
    const colors = new Float32Array(count * 4 * 3);
    const sizes = new Float32Array(count * 4);
    const alphas = new Float32Array(count * 4);
    const aspects = new Float32Array(count * 4);
    const rotations = new Float32Array(count * 4);
    const frames = new Float32Array(count * 4);
    const nextFrames = new Float32Array(count * 4);
    const frameMixes = new Float32Array(count * 4);
    const corners = new Float32Array(count * 4 * 2);
    const indices = new Uint16Array(count * 6);
    for (let i = 0; i < count; i++) {
      corners.set([-1, -1, 1, -1, -1, 1, 1, 1], i * 8);
      const base = i * 4;
      indices.set([base, base + 1, base + 2, base + 2, base + 1, base + 3], i * 6);
    }
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    for (const [name, array, width] of [
      ["position", positions, 3], ["aColor", colors, 3],
      ["aSize", sizes, 1], ["aAlpha", alphas, 1],
      ["aAspect", aspects, 1], ["aRotation", rotations, 1],
      ["aFrame", frames, 1], ["aFrameMix", frameMixes, 1],
      ["aNextFrame", nextFrames, 1],
      ["aCorner", corners, 2],
    ] as const) {
      const attribute = new THREE.BufferAttribute(array, width).setUsage(THREE.DynamicDrawUsage);
      geometry.setAttribute(name, attribute);
    }
    geometry.setDrawRange(0, 0);
    const tilesU = Math.max(1, emitter.particle.tileCount[0]);
    const tilesV = Math.max(1, emitter.particle.tileCount[1]);
    const material = new THREE.ShaderMaterial({
      uniforms: {
        uMap: { value: null }, uHasMap: { value: false },
        uTiles: { value: new THREE.Vector2(tilesU, tilesV) },
      },
      vertexShader: `
        attribute vec3 aColor;
        attribute float aSize;
        attribute float aAlpha;
        attribute float aAspect;
        attribute float aRotation;
        attribute float aFrame;
        attribute float aFrameMix;
        attribute float aNextFrame;
        attribute vec2 aCorner;
        varying vec3 vColor;
        varying float vAlpha;
        varying vec2 vUv;
        varying float vFrame;
        varying float vFrameMix;
        varying float vNextFrame;
        void main() {
          vColor = aColor;
          vAlpha = aAlpha;
          vUv = aCorner * 0.5 + 0.5;
          vFrame = aFrame;
          vFrameMix = aFrameMix;
          vNextFrame = aNextFrame;
          vec4 view = modelViewMatrix * vec4(position, 1.0);
          vec2 corner = vec2(aCorner.x * max(0.01, aAspect), aCorner.y) * aSize * 0.5;
          float sine = sin(aRotation);
          float cosine = cos(aRotation);
          view.xy += vec2(cosine * corner.x - sine * corner.y,
                          sine * corner.x + cosine * corner.y);
          gl_Position = projectionMatrix * view;
        }
      `,
      fragmentShader: `
        uniform sampler2D uMap;
        uniform bool uHasMap;
        uniform vec2 uTiles;
        varying vec3 vColor;
        varying float vAlpha;
        varying vec2 vUv;
        varying float vFrame;
        varying float vFrameMix;
        varying float vNextFrame;
        vec2 atlasUv(float frame) {
          float column = mod(frame, uTiles.x);
          float row = floor(frame / uTiles.x);
          return (vec2(column, row) + vUv) / uTiles;
        }
        void main() {
          float edge = 1.0 - smoothstep(0.42, 0.5, length(vUv - 0.5));
          vec4 texel = uHasMap
            ? mix(texture2D(uMap, atlasUv(vFrame)), texture2D(uMap, atlasUv(vNextFrame)), vFrameMix)
            : vec4(1.0, 1.0, 1.0, edge);
          gl_FragColor = vec4(vColor * texel.rgb, vAlpha * texel.a);
        }
      `,
      transparent: true,
      depthWrite: false,
      blending: [4, 5, 6].includes(emitter.particle.texture.drawMode) ? THREE.AdditiveBlending : THREE.NormalBlending,
    });
    let texture: ThreeNamespace.Texture | null = null;
    let disposed = false;
    if (emitter.texturePngBase64) {
      const image = new Image();
      image.onload = () => {
        if (disposed) return;
        texture = new THREE.Texture(image);
        texture.colorSpace = THREE.SRGBColorSpace;
        texture.needsUpdate = true;
        material.uniforms.uMap.value = texture;
        material.uniforms.uHasMap.value = true;
      };
      image.src = `data:image/png;base64,${emitter.texturePngBase64}`;
    }
    const quads = new THREE.Mesh(geometry, material);
    quads.frustumCulled = false;
    quads.matrix.copy(blockMatrix(THREE, emitter));
    quads.matrixAutoUpdate = false;
    object.add(quads);
    const solver = new ParticleSolver(
      emitter.particle, emitter.emission ?? emitter.particle,
      (unit.effectId?.instance ?? unit.index) ^ Math.imul(index + 1, 0x9e3779b1),
      count,
    );
    return {
      tick(delta: number) {
        solver.advance(delta);
        geometry.setDrawRange(0, solver.particles.length * 6);
        solver.particles.forEach((particle, i) => {
          const phase = particle.age / particle.life;
          const color = sampleColor(emitter.particle.color, phase);
          const frame = particleAtlasFrame(emitter.particle, particle);
          for (let corner = 0; corner < 4; corner++) {
            const vertex = i * 4 + corner;
            positions.set(particle.position, vertex * 3);
            colors.set(color, vertex * 3);
            sizes[vertex] = Math.max(0, sampleScalar(emitter.particle.size, phase));
            alphas[vertex] = Math.max(0, Math.min(1, sampleScalar(emitter.particle.alpha, phase)));
            aspects[vertex] = sampleScalar(emitter.particle.aspect, phase);
            rotations[vertex] = sampleScalar(emitter.particle.rotation, phase, 0) * Math.PI * 2;
            frames[vertex] = frame.current;
            nextFrames[vertex] = frame.next;
            frameMixes[vertex] = frame.mix;
          }
        });
        for (const name of ["position", "aColor", "aSize", "aAlpha", "aAspect", "aRotation", "aFrame", "aNextFrame", "aFrameMix"]) {
          geometry.getAttribute(name).needsUpdate = true;
        }
      },
      dispose() {
        disposed = true;
        geometry.dispose();
        material.dispose();
        texture?.dispose();
      },
    };
  });
  return {
    object,
    tick(delta) { views.forEach((view) => view.tick(delta)); },
    dispose() {
      views.forEach((view) => view.dispose());
      proxy.geometry.dispose();
      (proxy.material as ThreeNamespace.Material).dispose();
    },
  };
}
