/**
 * building4 浮雕 GLSL（发行版 `building4ClipAndReliefMapPS` 的 cone-step
 * 复刻）。由 `plan/relief` 的常量插值生成，避免着色器里散落魔法数字。
 */
import { RELIEF_CONSTANTS } from "../../plan/relief";

const cull = RELIEF_CONSTANTS.cullDistanceSq.toFixed(1);
const depth = RELIEF_CONSTANTS.depth.toFixed(3);
const cone = RELIEF_CONSTANTS.coneSteps;
const binary = RELIEF_CONSTANTS.binarySteps;
const maxCone = RELIEF_CONSTANTS.maxConeRatio.toFixed(2);
const flat = `${RELIEF_CONSTANTS.flatLevelInt}.0 / 255.0`;

export const RELIEF_GLSL = /* glsl */ `
// Building4 uses a cone-step relief pass for the facade height channel.
// Distance/flat-level handling matches the shipped fragment:
//   tmp/building_shaders_full.txt:896-935 (kReliefDepth=0.16,
//   kCullDistanceSq=16000, cone=8, binary=2, kFlatLevel=23/255).
vec2 scReliefUv(vec2 src, vec3 eye, sampler2D heightMap, vec4 region, vec2 dx, vec2 dy) {
  if (uReliefEnabled < 0.5) return src;
  float distSq = dot(vViewPosition, vViewPosition);
  if (distSq > ${cull}) return src;
  const float kReliefDepth = ${depth};
  const float kMaxConeRatio = ${maxCone};
  const float kFlatLevel = ${flat};
  const int kConeSteps = ${cone};
  const int kBinarySteps = ${binary};

  // Game divides the tangent-space XY ray by the Top atlas scale before
  // reliefMap(); keep that so dsRatio is in texture-space units.
  eye.xy /= max(abs(region.xy), vec2(1e-4));
  float safeZ = abs(eye.z) < 1e-4 ? (eye.z < 0.0 ? -1e-4 : 1e-4) : eye.z;
  vec3 ds = eye;
  // Source reliefMap() direction: height field is walked from camera to wall.
  ds.xy = -ds.xy;
  ds.z = safeZ;
  ds.z *= 1.0 / kReliefDepth;
  ds *= 1.0 / ds.z;
  src -= ds.xy * kFlatLevel;
  vec3 samplePos = vec3(src, 0.0);
  float dsRatio = length(ds.xy * region.xy);
  float finalDistance = 0.0;

  for (int i = 0; i < kConeSteps; i++) {
    vec2 sampleUv = clamp(samplePos.xy, 0.0, 1.0) * region.xy + region.zw;
    vec4 control = scSampleAtlas(heightMap, sampleUv, region, dx, dy);
    float height = control.r;
    float coneRatio = max(control.g * kMaxConeRatio, 0.0);
    float stepHeight = clamp(height - samplePos.z, 0.0, 1.0);
    float stepDistance = (dsRatio + coneRatio) > 1e-5
      ? (coneRatio * stepHeight) / (dsRatio + coneRatio)
      : 0.0;
    if (stepHeight > 0.0) finalDistance = stepDistance;
    samplePos += ds * stepDistance;
  }

  ds *= finalDistance;
  samplePos -= ds;
  for (int i = 0; i < kBinarySteps; i++) {
    ds *= 0.5;
    vec2 sampleUv = clamp(samplePos.xy, 0.0, 1.0) * region.xy + region.zw;
    float height = scSampleAtlas(heightMap, sampleUv, region, dx, dy).r;
    samplePos += ((height - samplePos.z) >= 0.0) ? ds : -ds;
  }

  // The shipped shader returns samplePos.xy without a flat early-out; the
  // flat level is applied as the tc offset above. Keep the unbounded result
  // so outsideTile can fall back to the Base layer.
  return samplePos.xy;
}
vec3 scReliefEyeDir() {
  vec3 tangentU;
  vec3 tangentV;
  vec3 normalV;
#ifdef USE_TANGENT
  // building4 VS uses the exported tangent and derives cross(tangent, normal).
  tangentU = normalize(vReliefTangent);
  normalV = normalize(vReliefNormal);
  tangentV = normalize(cross(tangentU, normalV));
#else
  // Foreign meshes without TANGENT.
  vec3 posV = -vViewPosition;
  vec3 dpx = dFdx(posV);
  vec3 dpy = dFdy(posV);
  vec2 dux = dFdx(vTopUv);
  vec2 duy = dFdy(vTopUv);
  float det = dux.x * duy.y - duy.x * dux.y;
  if (abs(det) < 1e-6) return vec3(0.0, 0.0, 1.0);
  tangentU = normalize((dpx * duy.y - dpy * dux.y) / det);
  normalV = normalize(vReliefNormal);
  tangentV = normalize(cross(tangentU, normalV));
#endif
  vec3 eyeV = normalize(vViewPosition);
  return normalize(vec3(dot(eyeV, tangentU), dot(eyeV, tangentV), dot(eyeV, normalV)));
}
`;
