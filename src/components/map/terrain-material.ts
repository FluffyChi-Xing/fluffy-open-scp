import * as THREE from "three";
import type { Region3DData } from "@/lib/region-map";
import { resourceColor } from "@/lib/region-map";
import { ecoChannel } from "./map-fields";
import { MAP_WORLD_XY_GLSL } from "./map-coordinates";
import { hasStateResource, stateResourcePixels, STATE_RESOURCE_IDS } from "./map-state-fields";

export type TerrainMaterial = THREE.MeshLambertMaterial & {
  setResource: (kind: string | null, opacity: number) => void;
};

/** Data fields stay in world space and at source resolution, independently of mesh LOD.
 * See docs/re/map-terrain-color-correction.md for the exporter/HLSL evidence.
 * Game diffuse faces use world-space triplanar projection; missing faces use a palette fallback.
 */
export function createTerrainMaterial(
  data: Region3DData,
  raw: Uint16Array,
  ecoPixels: Uint8ClampedArray,
  detailPixels: Partial<Record<"dirt" | "grass" | "cliff" | "sand", ImageData>> = {},
): TerrainMaterial {
  const size = Math.round(Math.sqrt(raw.length));
  const spacing = data.metersPerPixel;
  const normals = new Uint8Array(size * size * 4);
  // Use the source height field, not the decimated mesh: cliff masks must not
  // change when zooming or crossing a chunk/LOD boundary.
  for (let y = 0; y < size; y++) {
    const y0 = Math.max(0, y - 1);
    const y1 = Math.min(size - 1, y + 1);
    for (let x = 0; x < size; x++) {
      const x0 = Math.max(0, x - 1);
      const x1 = Math.min(size - 1, x + 1);
      const dx =
        (raw[y * size + x1] - raw[y * size + x0]) /
        (data.heightDiv * spacing * Math.max(1, x1 - x0));
      const dy =
        (raw[y1 * size + x] - raw[y0 * size + x]) /
        (data.heightDiv * spacing * Math.max(1, y1 - y0));
      const inv = 1 / Math.hypot(dx, 1, dy);
      const k = (y * size + x) * 4;
      normals[k] = Math.round((-dx * inv * 0.5 + 0.5) * 255);
      normals[k + 1] = Math.round((inv * 0.5 + 0.5) * 255);
      normals[k + 2] = Math.round((-dy * inv * 0.5 + 0.5) * 255);
      normals[k + 3] = 255;
    }
  }
  const field = (pixels: Uint8Array) => {
    const tex = new THREE.DataTexture(pixels, size, size, THREE.RGBAFormat);
    tex.colorSpace = THREE.NoColorSpace;
    tex.magFilter = THREE.LinearFilter;
    tex.minFilter = THREE.LinearFilter;
    tex.generateMipmaps = false;
    tex.needsUpdate = true;
    return tex;
  };
  const eco = field(new Uint8Array(ecoPixels));
  const normal = field(normals);
  const savedResources = Object.fromEntries(Object.keys(STATE_RESOURCE_IDS)
    .filter(kind => hasStateResource(data, kind)).map(kind => [kind, field(stateResourcePixels(data, kind, size))]));
  const detail = (slot: keyof typeof detailPixels, fallback: string) => {
    const px = detailPixels[slot];
    const color = new THREE.Color(fallback).convertLinearToSRGB();
    const tex = new THREE.DataTexture(
      px ? new Uint8Array(px.data) : new Uint8Array([color.r * 255, color.g * 255, color.b * 255, 255]),
      px?.width ?? 1, px?.height ?? 1, THREE.RGBAFormat,
    );
    tex.colorSpace = THREE.SRGBColorSpace;
    tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
    tex.magFilter = THREE.LinearFilter;
    tex.minFilter = THREE.LinearMipmapLinearFilter;
    tex.generateMipmaps = true;
    tex.anisotropy = 4;
    tex.needsUpdate = true;
    return tex;
  };
  const textures = {
    dirtMap: detail("dirt", "#ac946d"), grassMap: detail("grass", "#58813f"),
    cliffMap: detail("cliff", "#b7a07b"), sandMap: detail("sand", "#c5b588"),
  };
  const mat = new THREE.MeshLambertMaterial() as TerrainMaterial;
  const overlay = {
    resourceChannel: { value: -1 }, resourceOpacity: { value: 0.75 },
    resourceColor: { value: new THREE.Color() },
    savedResource: { value: eco },
  };
  mat.setResource = (kind, opacity) => {
    const saved = kind ? savedResources[kind] : undefined;
    overlay.savedResource.value = saved ?? eco;
    overlay.resourceChannel.value = saved ? 3 : ecoChannel(kind) ?? -1;
    overlay.resourceOpacity.value = Math.max(0, Math.min(1, opacity));
    overlay.resourceColor.value.set(resourceColor(kind ?? ""));
  };
  mat.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, {
      ...overlay,
      ...Object.fromEntries(Object.entries(textures).map(([key, value]) => [key, { value }])),
      terrainEco: { value: eco },
      terrainNormals: { value: normal },
      terrainOrigin: { value: new THREE.Vector2(...data.originWorld) },
      terrainExtent: { value: size * spacing },
      terrainTexel: { value: 0.5 / size },
      terrainWaterZ: { value: data.waterZ },
      dryColor: { value: new THREE.Color("#ac946d") },
      grassColor: { value: new THREE.Color("#58813f") },
      cliffColor: { value: new THREE.Color("#b7a07b") },
      sandColor: { value: new THREE.Color("#c5b588") },
      forestColor: { value: new THREE.Color("#395c38") },
    });
    shader.vertexShader = shader.vertexShader
      .replace(
        "#include <common>",
        `#include <common>\nvarying vec3 vTerrainWorld;\n${MAP_WORLD_XY_GLSL}`,
      )
      .replace(
        "#include <begin_vertex>",
        "#include <begin_vertex>\nvec3 sceneWorld = (modelMatrix * vec4(position, 1.0)).xyz;\nvTerrainWorld = vec3(mapWorldXY(sceneWorld).x, sceneWorld.y, mapWorldXY(sceneWorld).y);",
      );
    shader.fragmentShader = shader.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
varying vec3 vTerrainWorld;
uniform sampler2D terrainEco;
uniform sampler2D terrainNormals;
uniform sampler2D dirtMap, grassMap, cliffMap, sandMap;
uniform vec2 terrainOrigin;
uniform float terrainExtent;
uniform float terrainTexel;
uniform float terrainWaterZ;
uniform vec3 dryColor, grassColor, cliffColor, sandColor, forestColor;
uniform int resourceChannel;
uniform sampler2D savedResource;
uniform float resourceOpacity;
uniform vec3 resourceColor;
float terrainHash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float terrainNoise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(terrainHash(i), terrainHash(i + vec2(1, 0)), f.x),
    mix(terrainHash(i + vec2(0, 1)), terrainHash(i + vec2(1, 1)), f.x), f.y);
}
`,
      )
      .replace(
        "#include <color_fragment>",
        `#include <color_fragment>
vec2 worldXZ = vTerrainWorld.xz;
vec2 ecoUV = (worldXZ - terrainOrigin) / terrainExtent + terrainTexel;
vec3 eco = texture2D(terrainEco, ecoUV).rgb;
// Three is Y-up; the original HLSL uses terrainNormal.z in its Z-up world.
vec3 terrainNormal = normalize(texture2D(terrainNormals, ecoUV).rgb * 2.0 - 1.0);
float up = clamp(terrainNormal.y, 0.0, 1.0);
float flatness = up * up;
flatness *= flatness;
flatness *= flatness;
flatness *= flatness;
// getGrassAmount: sqrt(soil * water), patchy noise, normalUp^16.
// The noise texture/tuning are approximated; zero soil/water must stay bare.
float grass = sqrt(max(0.0, eco.r * eco.b));
float patchy = (terrainNoise(worldXZ / 96.0) - 0.5) * 0.16;
grass = sign(grass) * clamp(grass + patchy, 0.0, 1.0) * flatness;
float aboveWater = vTerrainWorld.y - terrainWaterZ;
float beachNoise = terrainNoise(worldXZ / 30.0) * 2.75 - 0.3875;
float drySand = clamp(terrainWaterZ + 0.3 + beachNoise - vTerrainWorld.y, 0.0, 1.0);
grass *= clamp(aboveWater - 2.0 - beachNoise + 1.0, 0.0, 1.0);
// World-metre UVs remain continuous across chunks and LODs. Periods are preview tuning.
vec2 detailUV = (worldXZ + 16384.0) / 64.0;
vec3 ground = mix(texture2D(dirtMap, detailUV).rgb, texture2D(grassMap, detailUV).rgb, grass);
ground = mix(ground, texture2D(sandMap, detailUV).rgb, drySand);
ground *= 1.0 - clamp(terrainWaterZ - 1.0 + beachNoise - vTerrainWorld.y, 0.0, 1.0) / 1.8;
// terrainPS triplanar weights: expose warm rock only on steep faces.
vec3 weights = max(abs(terrainNormal) - 0.15, vec3(0.0));
weights /= max(dot(weights, vec3(1.0)), 0.0001);
vec3 rockX = texture2D(cliffMap, (vTerrainWorld.zy + 16384.0) / 128.0).rgb;
vec3 rockZ = texture2D(cliffMap, (vTerrainWorld.xy + 16384.0) / 128.0).rgb;
vec3 surface = ground * weights.y + rockX * weights.x + rockZ * weights.z;
// Forest density is a separate channel. This is distant canopy-color detail,
// not a reconstruction of the game's individual tree impostors.
float canopy = smoothstep(0.40, 0.72, terrainNoise(worldXZ / 23.0));
float forest = eco.g * canopy * flatness * smoothstep(2.0, 8.0, aboveWater);
surface = mix(surface, forestColor, forest * 0.6);
float detail = terrainNoise(worldXZ / 8.0);
surface *= 0.95 + detail * 0.10;
diffuseColor.rgb = surface;
if (resourceChannel >= 0) {
  float amount = resourceChannel == 0 ? eco.r : resourceChannel == 1 ? eco.g : eco.b;
  vec4 saved = texture2D(savedResource, ecoUV);
  if (resourceChannel == 3) amount = saved.r * saved.a;
  float band = floor(clamp(amount, 0.0, 0.999) * 4.0) / 3.0;
  vec3 resourceTint = mix(vec3(0.85), resourceColor, 0.25 + band * 0.75);
  float coverage = smoothstep(0.0, 0.035, amount);
  diffuseColor.rgb = mix(surface, resourceTint, coverage * resourceOpacity);
}
`,
      );
  };
  mat.customProgramCacheKey = () => "region-game-diffuse-resource-v5-saved";
  mat.addEventListener("dispose", () => {
    eco.dispose();
    normal.dispose();
    Object.values(savedResources).forEach(tex => tex.dispose());
    Object.values(textures).forEach((tex) => tex.dispose());
  });
  return mat;
}
