import type { LotMaterialTextures } from "@/api/tauri";
import type { SurfaceMaterial, SurfaceUv } from "./propertyMaterial";

export interface AlphaMap {
  width: number;
  height: number;
  data: Uint8Array;
}
export interface BuildingAlphaMaps {
  tint: AlphaMap;
  shader: AlphaMap;
}

/** Only alpha is needed: canvas premultiplication cannot destroy these masks. */
async function decodeAlpha(bytes: Uint8Array<ArrayBuffer>): Promise<AlphaMap> {
  const bitmap = await createImageBitmap(
    new Blob([bytes], { type: "image/png" }),
  );
  try {
    const canvas = document.createElement("canvas");
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) throw new Error("Cannot read building material masks");
    context.drawImage(bitmap, 0, 0);
    const rgba = context.getImageData(0, 0, canvas.width, canvas.height).data;
    const data = new Uint8Array(canvas.width * canvas.height);
    for (let i = 0; i < data.length; i++) data[i] = rgba[i * 4 + 3];
    return { width: canvas.width, height: canvas.height, data };
  } finally {
    bitmap.close();
  }
}

export async function readBuildingAlphaMaps(
  material: LotMaterialTextures,
): Promise<BuildingAlphaMaps> {
  const [tint, shader] = await Promise.all([
    decodeAlpha(material.tintPng!),
    decodeAlpha(material.shaderPng!),
  ]);
  return { tint, shader };
}

const clamp = (value: number, low: number, high: number) =>
  Math.max(low, Math.min(high, value));
const fract = (value: number) => value - Math.floor(value);

/** Match scSampleAtlas at mip 0, including signed regions and half-texel clamp.
 * Building4 textures use flipY=false, so PNG row zero is UV y=0. */
function sampleAlpha(
  map: AlphaMap,
  uv: number[],
  region: ArrayLike<number>,
): number {
  const xy = [map.width, map.height].map((size, axis) => {
    const a = region[axis + 2],
      b = a + region[axis];
    const inset = Math.min(0.5 / size, Math.abs(region[axis]) / 2);
    return clamp(
      clamp(uv[axis], Math.min(a, b) + inset, Math.max(a, b) - inset) * size -
        0.5,
      0,
      size - 1,
    );
  });
  const [x, y] = xy,
    ix = Math.floor(x),
    iy = Math.floor(y);
  const jx = Math.min(ix + 1, map.width - 1),
    jy = Math.min(iy + 1, map.height - 1);
  const at = (px: number, py: number) => map.data[py * map.width + px] / 255;
  const mix = (a: number, b: number, t: number) => a + (b - a) * t;
  return mix(
    mix(at(ix, iy), at(jx, iy), x - ix),
    mix(at(ix, jy), at(jx, jy), x - ix),
    y - iy,
  );
}

/** Estimate visible windows on the transferred UV mapping, not just the atlas.
 * A 32m square covers ordinary facades while retaining constant/knife-band UVs.
 * Jitter prevents integer tile periods from aliasing with the sampling grid.
 * This is a selection heuristic, not a replacement for the production shader. */
export function measureFacadeCoverage(
  material: LotMaterialTextures,
  surface: SurfaceMaterial,
  maps: BuildingAlphaMaps,
): { windowCoverage: number; topCoverage: number } {
  const row = (r: number) =>
    material.paramsF32!.subarray(
      (r * material.paramCols + surface.column) * 4,
      (r * material.paramCols + surface.column + 1) * 4,
    );
  const base = row(1),
    top = row(2),
    room = row(3);
  if (![...base, ...top, ...room].every(Number.isFinite))
    return { windowCoverage: 0, topCoverage: 0 };
  const hasTop = Math.abs(top[0]) > 1e-6 && Math.abs(top[1]) > 1e-6;
  let windows = 0,
    overlay = 0;
  const resolution = 48;
  const project = (mapping: SurfaceUv, x: number, y: number) =>
    mapping.offset.map((offset, axis) =>
      fract(offset + mapping.horizontal[axis] * x + mapping.vertical[axis] * y),
    );
  for (let y = 0; y < resolution; y++) {
    for (let x = 0; x < resolution; x++) {
      const s = ((x + fract((y + 1) * 0.754877666)) / resolution - 0.5) * 32;
      const t = ((y + fract((x + 1) * 0.569840296)) / resolution - 0.5) * 32;
      const buv = project(surface.base, s, t).map(
        (v, axis) => v * base[axis] + base[axis + 2],
      );
      // Base alpha clips the whole fragment, even when Top contains a window.
      if (sampleAlpha(maps.tint, buv, base) < 0.5) continue;
      let opacity = sampleAlpha(maps.shader, buv, base);
      if (hasTop) {
        const relief = project(surface.top, s, t).map(
          (v, axis) => v * (1 + room[axis]) - room[axis] / 2,
        );
        if (relief.every((v) => v >= 0 && v <= 1)) {
          const tuv = relief.map((v, axis) => v * top[axis] + top[axis + 2]);
          const weight = sampleAlpha(maps.tint, tuv, top);
          overlay += weight;
          opacity += (sampleAlpha(maps.shader, tuv, top) - opacity) * weight;
        }
      }
      windows += 1 - opacity;
    }
  }
  return {
    windowCoverage: windows / resolution ** 2,
    topCoverage: overlay / resolution ** 2,
  };
}
