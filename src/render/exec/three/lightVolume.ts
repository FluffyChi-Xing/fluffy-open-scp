import type * as ThreeNamespace from "three";

/**
 * Spot-light volumetric cone (game parity, building-light pass).
 *
 * The shipped shader (`tmp/shaders/cpp_SimCity_App_g40212002_0.txt:5278-5293`
 * and the `intersectRayCone` block at `:5304`) is not a textured cone: the
 * vertex shader intersects the camera ray with the light cone
 * (`coneHeight = falloffDist`, `cosHalfAngle = radius * 0.97`) and the pixel
 * shader marches the in-volume span, accumulating the light falloff:
 *
 * ```
 * tValues   = intersectRayCone(lightPos, -lightDir, cameraPos, rayDir,
 *                              falloffDist, cosHalfAngle)
 * span      = tValues.y - tValues.x
 * fogAtten  = average(saturate(falloffDist² - d²) / falloffDist²) over span
 * colour    = fogColor * diffLightColor * fogNoise * fogAtten
 * ```
 *
 * We keep the same idea per fragment (no vertex interpolation error): solve
 * the ray/truncated-cone interval analytically in object space and integrate
 * a linear-in-distance-squared falloff along it. The result is an additive
 * shell whose brightness is proportional to how much light volume the eye ray
 * traverses — i.e. a real cone of light, brightest through the core and
 * fading to zero at the silhouette, instead of a solid triangle.
 */

export interface LightVolumeOptions {
  readonly color: ThreeNamespace.ColorRepresentation;
  /** Cone height along local +Y (metres). */
  readonly length: number;
  /** Cone end radius at `length` (metres). */
  readonly radius: number;
  /** Overall beam strength (diffuse × volumetric strength). */
  readonly intensity: number;
}

export interface LightVolumeMesh {
  readonly mesh: ThreeNamespace.Mesh;
  /** Per-frame camera uniform refresh; call from onBeforeRender (set here). */
  readonly material: ThreeNamespace.ShaderMaterial;
}

const CONE_SEGMENTS = 32;

const VERTEX_SHADER = /* glsl */ `
varying vec3 vLocal;
void main() {
  vLocal = position;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`;

/**
 * `uTan2` = (radius/length)^2, `uFar2` = length^2 + radius^2 (distance from the
 * apex to the base rim). The ray/cone solve is in object space; the apex is at
 * the origin and the cone opens along +Y, y ∈ [0, length].
 */
const FRAGMENT_SHADER = /* glsl */ `
precision highp float;
uniform vec3 uColor;
uniform vec3 uCameraLocal;
uniform float uHeight;
uniform float uTan2;
uniform float uFar2;
uniform float uIntensity;
varying vec3 vLocal;

void coneSpan(vec3 ro, vec3 rd, out float t0, out float t1) {
  float a = rd.x * rd.x + rd.z * rd.z - uTan2 * rd.y * rd.y;
  float b = 2.0 * (ro.x * rd.x + ro.z * rd.z - uTan2 * ro.y * rd.y);
  float c = ro.x * ro.x + ro.z * ro.z - uTan2 * ro.y * ro.y;
  float tA = -1e20;
  float tB = 1e20;
  if (abs(a) > 1e-7) {
    float disc = b * b - 4.0 * a * c;
    if (disc < 0.0) {
      t0 = 0.0;
      t1 = 0.0;
      return;
    }
    float s = sqrt(disc);
    tA = (-b - s) / (2.0 * a);
    tB = (-b + s) / (2.0 * a);
    if (tA > tB) {
      float tmp = tA;
      tA = tB;
      tB = tmp;
    }
  } else {
    if (abs(b) < 1e-9) {
      t0 = 0.0;
      t1 = 0.0;
      return;
    }
    tA = -c / b;
    tB = tA;
  }
  // Clamp the infinite double cone to the forward cone y ∈ [0, height].
  float tyA = -1e20;
  float tyB = 1e20;
  if (abs(rd.y) > 1e-7) {
    tyA = (0.0 - ro.y) / rd.y;
    tyB = (uHeight - ro.y) / rd.y;
    if (tyA > tyB) {
      float tmp = tyA;
      tyA = tyB;
      tyB = tmp;
    }
  } else if (ro.y < 0.0 || ro.y > uHeight) {
    t0 = 0.0;
    t1 = 0.0;
    return;
  }
  t0 = max(max(tA, tyA), 0.0);
  t1 = min(tB, tyB);
  if (t1 <= t0) {
    t0 = 0.0;
    t1 = 0.0;
  }
}

void main() {
  vec3 ro = uCameraLocal;
  vec3 delta = vLocal - ro;
  if (dot(delta, delta) < 1e-10) discard;
  vec3 rd = normalize(delta);
  float t0;
  float t1;
  coneSpan(ro, rd, t0, t1);
  float span = t1 - t0;
  if (span <= 0.0) discard;

  // Game falloff is linear in distance² from the light. Integrate it along
  // the visible span so the core is bright and the far end fades out.
  const int STEPS = 6;
  float atten = 0.0;
  for (int i = 0; i < STEPS; i++) {
    float t = t0 + span * (float(i) + 0.5) / float(STEPS);
    vec3 p = ro + rd * t;
    atten += clamp(1.0 - dot(p, p) / uFar2, 0.0, 1.0);
  }
  atten /= float(STEPS);

  // sqrt(span) keeps the core visible without blowing out long beams.
  float alpha = clamp(sqrt(span) * uIntensity * atten, 0.0, 1.0);
  if (alpha <= 0.001) discard;
  gl_FragColor = vec4(uColor, alpha);
}`;

/** Build the additive volumetric shell for one spot light (local +Y). */
export function createLightVolumeMesh(
  THREE: typeof ThreeNamespace,
  options: LightVolumeOptions,
): LightVolumeMesh {
  const length = Math.max(options.length, 0.25);
  const radius = Math.max(options.radius, 0.05);
  const geometry = new THREE.CylinderGeometry(radius, 0.0001, length, CONE_SEGMENTS, 1, true);
  geometry.translate(0, length / 2, 0);

  const material = new THREE.ShaderMaterial({
    uniforms: {
      uColor: { value: new THREE.Color(options.color) },
      uCameraLocal: { value: new THREE.Vector3() },
      uHeight: { value: length },
      uTan2: { value: (radius / length) * (radius / length) },
      uFar2: { value: length * length + radius * radius },
      uIntensity: { value: Math.max(options.intensity, 0.01) },
    },
    vertexShader: VERTEX_SHADER,
    fragmentShader: FRAGMENT_SHADER,
    transparent: true,
    depthWrite: false,
    depthTest: true,
    // Back faces alone cover the silhouette for a convex open cone, so each
    // covered pixel contributes exactly one (already integrated) volume term.
    side: THREE.BackSide,
    blending: THREE.AdditiveBlending,
    toneMapped: false,
  });

  const mesh = new THREE.Mesh(geometry, material);
  mesh.name = "__lightVolume";
  // The beam is editor chrome: never a pick target and never a shadow caster
  // (ThreeViewer.setShadowsEnabled skips pickOnly meshes).
  mesh.userData.pickOnly = true;
  mesh.raycast = () => {};
  const scratch = new THREE.Vector3();
  mesh.onBeforeRender = (_renderer, _scene, camera) => {
    camera.getWorldPosition(scratch);
    mesh.worldToLocal(scratch);
    material.uniforms.uCameraLocal.value.copy(scratch);
  };
  return { mesh, material };
}

/** Beam strength: diffuse level (game 0..1+) × authored volumetric strength. */
export function lightVolumeIntensity(
  diffuse: number | null | undefined,
  volumetricStrength: number | null | undefined,
): number {
  const d = Math.max(diffuse ?? 1, 0.05);
  const s = Math.max(volumetricStrength ?? 1, 0.05);
  // 0.12 keeps the additive core readable without washing the facade out.
  return Math.min(0.12 * d * s, 0.6);
}
