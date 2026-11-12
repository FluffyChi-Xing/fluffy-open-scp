import type * as ThreeNamespace from "three";

type Three = typeof ThreeNamespace;
const MAX_LINE_LIGHTS = 32;

interface LineLight {
  length: number;
  radius: number;
  innerRadius: number;
  diffuse: number;
  specular: number;
  color: [number, number, number];
}

export function setLineLight(object: ThreeNamespace.Object3D, light: LineLight): void {
  object.userData.lineLight = light;
}

function createUniforms(THREE: Three) {
  return {
    scLineCount: { value: 0 },
    scLineStartRadius: { value: Array.from({ length: MAX_LINE_LIGHTS }, () => new THREE.Vector4()) },
    scLineAxisLength: { value: Array.from({ length: MAX_LINE_LIGHTS }, () => new THREE.Vector4()) },
    scLineColorInner: { value: Array.from({ length: MAX_LINE_LIGHTS }, () => new THREE.Vector4()) },
    scLineLevels: { value: Array.from({ length: MAX_LINE_LIGHTS }, () => new THREE.Vector2()) },
  };
}

// Material hooks can be shared between viewers. Each renderer owns its uniforms.
const uniformsByRenderer = new WeakMap<ThreeNamespace.WebGLRenderer, ReturnType<typeof createUniforms>>();
const patchedMaterials = new WeakSet<ThreeNamespace.Material>();

const DECLARATIONS = /* glsl */ `
uniform int scLineCount;
uniform vec4 scLineStartRadius[${MAX_LINE_LIGHTS}];
uniform vec4 scLineAxisLength[${MAX_LINE_LIGHTS}];
uniform vec4 scLineColorInner[${MAX_LINE_LIGHTS}];
uniform vec2 scLineLevels[${MAX_LINE_LIGHTS}];
`;

// SimCity_App g40212002_0, Line VF 4846–4870 and linePF 24488–24507.
// Preserve the original cylinder attenuation and feed the incident direction
// into the receiving Three material's BRDF with independent Kd / Ks.
const CONTRIBUTION = /* glsl */ `
for (int i = 0; i < scLineCount; ++i) {
  vec4 startRadius = scLineStartRadius[i];
  vec4 axisLength = scLineAxisLength[i];
  vec4 colorInner = scLineColorInner[i];
  vec3 fromStart = geometryPosition - startRadius.xyz;
  float axial = dot(fromStart, axisLength.xyz);
  float halfLength = axisLength.w * 0.5;
  float endCap = pow(clamp((halfLength - abs(halfLength - axial)) /
    (halfLength - halfLength * 0.65), 0.0, 1.0), 1.5);
  if (endCap <= 0.0) continue;
  vec3 toAxis = axisLength.xyz * axial - fromStart;
  float distance2 = dot(toAxis, toAxis);
  float radial = clamp((startRadius.w - distance2) /
    max(startRadius.w - colorInner.w, 1e-6), 0.0, 1.0);
  if (radial <= 0.0 || distance2 <= 1e-12) continue;
  IncidentLight lineIncident;
  lineIncident.direction = toAxis * inversesqrt(distance2);
  lineIncident.color = colorInner.rgb * (radial * radial * endCap);
  lineIncident.visible = true;
  ReflectedLight lineReflection = ReflectedLight(vec3(0.0), vec3(0.0), vec3(0.0), vec3(0.0));
  RE_Direct(lineIncident, geometryPosition, geometryNormal, geometryViewDir,
    geometryClearcoatNormal, material, lineReflection);
  outgoingLight += lineReflection.directDiffuse * scLineLevels[i].x
    + lineReflection.directSpecular * scLineLevels[i].y;
}
`;

function bindMaterial(THREE: Three, material: ThreeNamespace.Material): void {
  if (patchedMaterials.has(material)) return;
  if (!(material instanceof THREE.MeshStandardMaterial ||
    material instanceof THREE.MeshLambertMaterial ||
    material instanceof THREE.MeshPhongMaterial)) return;

  const compile = material.onBeforeCompile;
  const cacheKey = material.customProgramCacheKey;
  const defaultKey = cacheKey === THREE.Material.prototype.customProgramCacheKey;
  const initialKey = defaultKey ? cacheKey.call(material) : "";
  material.onBeforeCompile = function (shader, renderer) {
    compile.call(this, shader, renderer);
    if (!shader.fragmentShader.includes("#include <lights_fragment_begin>") ||
      !shader.fragmentShader.includes("#include <opaque_fragment>")) return;
    let uniforms = uniformsByRenderer.get(renderer);
    if (!uniforms) {
      uniforms = createUniforms(THREE);
      uniformsByRenderer.set(renderer, uniforms);
    }
    Object.assign(shader.uniforms, uniforms);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", `#include <common>\n${DECLARATIONS}`)
      .replace("#include <opaque_fragment>", `${CONTRIBUTION}\n#include <opaque_fragment>`);
  };
  material.customProgramCacheKey = function () {
    return `${defaultKey ? initialKey : cacheKey.call(this)}+sc-line-light`;
  };
  material.needsUpdate = true;
  patchedMaterials.add(material);
}

export function installLineLightPass(
  THREE: Three,
  scene: ThreeNamespace.Scene,
  renderer: ThreeNamespace.WebGLRenderer,
  getLights: () => ThreeNamespace.Object3D | undefined,
): void {
  const uniforms = createUniforms(THREE);
  uniformsByRenderer.set(renderer, uniforms);
  const start = new THREE.Vector3();
  const axis = new THREE.Vector3();
  const beforeRender = scene.onBeforeRender;
  scene.onBeforeRender = function (...args) {
    beforeRender.apply(this, args);
    const camera = args[2];
    let count = 0;
    getLights()?.traverseVisible((object) => {
      const light = object.userData.lineLight as LineLight | undefined;
      if (!light || count >= MAX_LINE_LIGHTS) return;
      start.setFromMatrixPosition(object.matrixWorld).applyMatrix4(camera.matrixWorldInverse);
      axis.set(0, light.length, 0).applyMatrix4(object.matrixWorld)
        .applyMatrix4(camera.matrixWorldInverse).sub(start);
      const length = axis.length();
      if (length <= 1e-6) return;
      axis.divideScalar(length);
      uniforms.scLineStartRadius.value[count].set(start.x, start.y, start.z, light.radius ** 2);
      uniforms.scLineAxisLength.value[count].set(axis.x, axis.y, axis.z, length);
      uniforms.scLineColorInner.value[count].set(...light.color, light.innerRadius ** 2);
      uniforms.scLineLevels.value[count].set(light.diffuse, light.specular);
      count += 1;
    });
    uniforms.scLineCount.value = count;
    if (count === 0) return;
    scene.traverseVisible((object) => {
      const mesh = object as ThreeNamespace.Mesh;
      if (!mesh.isMesh) return;
      for (const material of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
        bindMaterial(THREE, material);
      }
    });
  };
}
