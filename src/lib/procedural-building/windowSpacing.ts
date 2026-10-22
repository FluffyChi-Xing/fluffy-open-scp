import type { Vector3 } from "three";
import type { SolidFace } from "./solidGeometry";
import type {
  BuildingMaterialSource,
  SurfaceMaterial,
  SurfaceUv,
} from "./propertyMaterial";

/** Insert physical wall strips between complete horizontal atlas repeats.
 * UV derivatives inside a repeat stay unchanged: windows are never stretched.
 * A repeat can contain several windows in the source artwork. */
export function spacedFacadeFaces(
  face: SolidFace,
  source: BuildingMaterialSource,
  spacing: number,
): { face: SolidFace; surface: SurfaceMaterial }[] {
  if (Math.abs(face.normal.z) > 0.7) return [{ face, surface: source.roof }];
  if (!Number.isFinite(spacing) || spacing <= 0)
    return [{ face, surface: source.facade }];
  const facade = source.facade;
  const mapping = facade.topCoverage > 0.01 ? facade.top : facade.base;
  const axis =
    Math.abs(mapping.horizontal[0]) >= Math.abs(mapping.horizontal[1]) ? 0 : 1;
  const slope = mapping.horizontal[axis];
  // Constant source UVs have no identifiable repeat. Leave them intact.
  if (Math.abs(slope) < 1e-5) return [{ face, surface: facade }];
  const width = Math.max(0.25, 1 / Math.abs(slope));
  const gap = Math.min(12, spacing);
  const period = width + gap;
  const anchor = -mapping.offset[axis] / slope;
  const h = (p: Vector3) => -face.normal.y * p.x + face.normal.x * p.y;
  const values = face.vertices.map(h);
  const min = Math.min(...values),
    max = Math.max(...values);
  const clip = (vertices: Vector3[], limit: number, above: boolean) => {
    const result: Vector3[] = [];
    vertices.forEach((p, i) => {
      const q = vertices[(i + 1) % vertices.length];
      const a = (h(p) - limit) * (above ? 1 : -1);
      const b = (h(q) - limit) * (above ? 1 : -1);
      if (a >= 0) result.push(p);
      if (a < 0 !== b < 0) result.push(p.clone().lerp(q, a / (a - b)));
    });
    return result;
  };
  const pieces: { face: SolidFace; surface: SurfaceMaterial }[] = [];
  const push = (lo: number, hi: number, surface: SurfaceMaterial) => {
    if (hi <= min + 1e-7 || lo >= max - 1e-7) return;
    const vertices = clip(clip(face.vertices, lo, true), hi, false);
    if (vertices.length >= 3)
      pieces.push({ face: { vertices, normal: face.normal }, surface });
  };
  for (
    let cycle = Math.floor((min - anchor) / period);
    cycle <= Math.floor((max - anchor) / period);
    cycle++
  ) {
    const start = anchor + cycle * period;
    const shift = (uv: SurfaceUv): SurfaceUv => ({
      ...uv,
      offset: uv.offset.map((v, i) => v - uv.horizontal[i] * cycle * gap) as [
        number,
        number,
      ],
    });
    push(start, start + width, {
      ...facade,
      base: shift(facade.base),
      top: shift(facade.top),
    });
    push(start + width, start + period, source.wall ?? source.roof);
  }
  return pieces;
}
