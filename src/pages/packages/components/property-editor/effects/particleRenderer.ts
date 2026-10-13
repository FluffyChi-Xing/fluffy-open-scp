import type * as ThreeNamespace from "three";
import type { EffectEmitterData, EffectUnit } from "@/api/tauri";
import { unitMatrix } from "../unitGizmos";
import { ParticleSolver, sampleColor, sampleScalar } from "./particleSolver";

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
    const positions = new Float32Array(count * 3);
    const colors = new Float32Array(count * 3);
    const sizes = new Float32Array(count);
    const alphas = new Float32Array(count);
    for (const [name, array, width] of [
      ["position", positions, 3], ["aColor", colors, 3],
      ["aSize", sizes, 1], ["aAlpha", alphas, 1],
    ] as const) {
      const attribute = new THREE.BufferAttribute(array, width).setUsage(THREE.DynamicDrawUsage);
      geometry.setAttribute(name, attribute);
    }
    geometry.setDrawRange(0, 0);
    const material = new THREE.ShaderMaterial({
      uniforms: { uMap: { value: null }, uHasMap: { value: false } },
      vertexShader: `
        attribute vec3 aColor;
        attribute float aSize;
        attribute float aAlpha;
        varying vec3 vColor;
        varying float vAlpha;
        void main() {
          vColor = aColor;
          vAlpha = aAlpha;
          vec4 view = modelViewMatrix * vec4(position, 1.0);
          gl_Position = projectionMatrix * view;
          gl_PointSize = clamp(aSize * 350.0 / max(1.0, -view.z), 1.0, 96.0);
        }
      `,
      fragmentShader: `
        uniform sampler2D uMap;
        uniform bool uHasMap;
        varying vec3 vColor;
        varying float vAlpha;
        void main() {
          vec2 uv = gl_PointCoord;
          float edge = 1.0 - smoothstep(0.42, 0.5, length(uv - 0.5));
          vec4 texel = uHasMap ? texture2D(uMap, uv) : vec4(1.0, 1.0, 1.0, edge);
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
    const points = new THREE.Points(geometry, material);
    points.frustumCulled = false;
    points.matrix.copy(blockMatrix(THREE, emitter));
    points.matrixAutoUpdate = false;
    object.add(points);
    const solver = new ParticleSolver(
      emitter.particle, emitter.emission ?? emitter.particle,
      (unit.effectId?.instance ?? unit.index) ^ Math.imul(index + 1, 0x9e3779b1),
      count,
    );
    return {
      tick(delta: number) {
        solver.advance(delta);
        geometry.setDrawRange(0, solver.particles.length);
        solver.particles.forEach((particle, i) => {
          const phase = particle.age / particle.life;
          const color = sampleColor(emitter.particle.color, phase);
          positions.set(particle.position, i * 3);
          colors.set(color, i * 3);
          sizes[i] = Math.max(0, sampleScalar(emitter.particle.size, phase) * Math.abs(emitter.transform.scale));
          alphas[i] = Math.max(0, Math.min(1, sampleScalar(emitter.particle.alpha, phase)));
        });
        for (const name of ["position", "aColor", "aSize", "aAlpha"]) {
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
