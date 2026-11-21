import type * as Three from "three";
import type { DecalFrameObject } from "./decal";

/**
 * decalProject[374] samples the receiver, but decalFloatQuad[257] supplies
 * decalInteriorMap[380] with the rasterized cube position. Emulate the latter
 * by intersecting the receiver's viewing ray with the far cube face. This is
 * the decal's own cube, not building4's room/interior-map algorithm.
 * See SC/cDecalData/SpawnDecalInstance.c:125-138, 246-310.
 */
export function configureHoleDecal(
  THREE: typeof Three,
  material: Three.ShaderMaterial,
  frame: DecalFrameObject,
  depth: number,
): void {
  material.fragmentShader = material.fragmentShader.replace(/\r\n/g, "\n");
  const toBox = new THREE.Matrix4().makeScale(
    2 / frame.sizeX, 2 / frame.sizeY, 2 / depth,
  ).multiply(frame.matrix.clone().invert());
  toBox.elements[14] -= 1;
  material.uniforms.uHoleToBox = { value: toBox };
  material.uniforms.uHoleCameraBox = { value: new THREE.Vector3() };
  material.uniforms.uHoleOrthoRay = { value: new THREE.Vector3() };
  material.uniforms.uHoleOrthographic = { value: false };
  material.vertexShader = `
varying vec3 vTexcoord0;
varying vec4 vTexcoord4;
varying vec4 vTexcoord5;
varying vec2 vUv;
uniform mat4 uHoleToBox;
void main() {
  vTexcoord0 = (uHoleToBox * vec4(position, 1.0)).xyz;
  vUv = vTexcoord0.xy * -0.5 + 0.5;
  vTexcoord4 = vec4(0.0);
  vTexcoord5 = vec4(0.0);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`;
  const legacy = `vec3 textureFloatPosition = vTexcoord0.xyz;
// decalClip[375]：只保留投影盒 z >= 0 的半空间。
if (textureFloatPosition.z < 0.0) discard;`;
  // Fail closed if the generated source changes: never silently reintroduce
  // the single-coordinate / half-volume implementation.
  if (!material.fragmentShader.includes(legacy)) {
    throw new Error("Hole decal shader contract changed: dual coordinates required");
  }
  material.fragmentShader = material.fragmentShader.replace(
    "void main() {", "uniform vec3 uHoleCameraBox;\nuniform vec3 uHoleOrthoRay;\nuniform bool uHoleOrthographic;\nvoid main() {",
  ).replace(legacy, `
// decalClip[375] clips the receiver against the ENTIRE normalized cube.
if (any(greaterThan(abs(vTexcoord0), vec3(1.00001)))) discard;
vec3 ray = uHoleOrthographic ? uHoleOrthoRay : vTexcoord0 - uHoleCameraBox;
// A parallel slab has an infinite exit distance. Avoid 0/0 at cube edges.
vec3 exitDistance = vec3(1.0e20);
if (abs(ray.x) > 1.0e-7) exitDistance.x = (sign(ray.x) - vTexcoord0.x) / ray.x;
if (abs(ray.y) > 1.0e-7) exitDistance.y = (sign(ray.y) - vTexcoord0.y) / ray.y;
if (abs(ray.z) > 1.0e-7) exitDistance.z = (sign(ray.z) - vTexcoord0.z) / ray.z;
float exitT = max(0.0, min(exitDistance.x, min(exitDistance.y, exitDistance.z)));
vec3 textureFloatPosition = clamp(vTexcoord0 + ray * exitT, -1.0, 1.0);`);
  const inverseWorld = new THREE.Matrix4();
  const cameraPoint = new THREE.Vector3();
  material.onBeforeRender = (_renderer, _scene, camera, _geometry, object) => {
    inverseWorld.copy(object.matrixWorld).invert();
    // Orthographic views need parallel rays, not rays from the eye point.
    material.uniforms.uHoleOrthographic.value =
      !!(camera as Three.OrthographicCamera).isOrthographicCamera;
    camera.getWorldDirection(material.uniforms.uHoleOrthoRay.value);
    material.uniforms.uHoleOrthoRay.value.transformDirection(inverseWorld).transformDirection(toBox);
    camera.getWorldPosition(cameraPoint);
    material.uniforms.uHoleCameraBox.value.copy(cameraPoint)
      .applyMatrix4(inverseWorld).applyMatrix4(toBox);
    material.uniformsNeedUpdate = true;
  };
  material.needsUpdate = true;
}
