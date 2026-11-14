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
  // Derive the relief frame from the **Top/facade UV derivatives**, not from
  // the glTF TANGENT (which is authored for the base UV): the same wall can
  // mix several Top-UV islands with different orientation.
  vec3 posV = -vViewPosition;
  vec3 dpx = dFdx(posV);
  vec3 dpy = dFdy(posV);
  vec2 dux = dFdx(vTopUv);
  vec2 duy = dFdy(vTopUv);
  // det < 0 = the Top-UV island is mirrored (U or V). This sign is part of
  // the parameterisation and MUST reach the V axis; otherwise mirrored
  // window columns pop out while their neighbours pop in.
  float det = dux.x * duy.y - duy.x * dux.y;
  vec3 eyeV = normalize(vViewPosition);

  // Anchor the relief depth axis to the rasterized face: cross(dPdx, dPdy)
  // always points toward the camera for a visible fragment. This normalises
  // flipped imported normals AND DoubleSide back faces to one side, so ds.z
  // never changes sign (a negative ds.z inverts the whole relief).
  // An extra front-facing flip here used to cancel this alignment on back
  // faces, making the relief direction depend on which side was visible.
  vec3 normalV = normalize(vReliefNormal);
  vec3 geomCross = cross(dpx, dpy);
  float geomLen = length(geomCross);
  if (geomLen > 1e-8) {
    vec3 geometryNormal = geomCross / geomLen;
    if (dot(normalV, geometryNormal) < 0.0) normalV = -normalV;
  } else if (dot(normalV, eyeV) < 0.0) {
    normalV = -normalV;
  }

  if (abs(det) < 1e-6) return vec3(0.0, 0.0, 1.0);
  vec3 tangentU = normalize((dpx * duy.y - dpy * dux.y) / det);
  // Keep the frame orthonormal like building4 (exported T/N are orthonormal);
  // guard the degenerate case so a bad normal cannot produce NaN.
  vec3 tangentOrtho = tangentU - normalV * dot(tangentU, normalV);
  if (length(tangentOrtho) > 1e-6) tangentU = normalize(tangentOrtho);
  // dP/dv = cross(N, dP/du) for det > 0, and its negation for a mirrored
  // island. The game gets this handedness from the exported tangent; we have
  // to take it from the UV determinant (same formula as the interior frame).
  vec3 tangentV = cross(normalV, tangentU) * (det < 0.0 ? -1.0 : 1.0);
  return normalize(vec3(dot(eyeV, tangentU), dot(eyeV, tangentV), dot(eyeV, normalV)));
}
`;
