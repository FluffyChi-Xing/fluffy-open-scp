import * as THREE from "three";
import type { LotModelPayload, Tgi } from "@/api/tauri";
import { parseLotModelObjects } from "@/lib/three-gltf";
import {
  measureFacadeCoverage,
  readBuildingAlphaMaps,
  type BuildingAlphaMaps,
} from "./facadeCoverage";

/** UV = offset + horizontal * s + vertical * t, in metres, not per box. */
export interface SurfaceUv {
  offset: [number, number];
  horizontal: [number, number];
  vertical: [number, number];
}
export interface SurfaceMaterial {
  materialIndex: number;
  column: number;
  seed: number;
  base: SurfaceUv;
  top: SurfaceUv;
  /** Estimated fragment coverage of window/interior pixels on the transferred facade. */
  windowCoverage: number;
  /** Estimated fraction where the Top (window motif) layer is active. */
  topCoverage: number;
}
export interface BuildingMaterialSource {
  payload: LotModelPayload;
  facade: SurfaceMaterial;
  roof: SurfaceMaterial;
  /** Verified opaque facade candidate for inserted window gaps. */
  wall?: SurfaceMaterial;
}

export function parsePropertyTgi(text: string): Tgi | null {
  const match = /^([\da-f]{8}):([\da-f]{8}):([\da-f]{8})$/i.exec(text.trim());
  if (!match) return null;
  const [typeId, group, instance] = match
    .slice(1)
    .map((value) => Number.parseInt(value, 16));
  return typeId === 0x00b1b104 ? { typeId, group, instance } : null;
}

export function isBuildingMaterial(
  material: LotModelPayload["materials"][number],
): boolean {
  return !!(
    material.tintPng?.length &&
    material.normalPng?.length &&
    material.palettePng?.length &&
    material.shaderPng?.length &&
    material.interiorPng?.length &&
    material.paramCols > 0 &&
    material.paramsF32 &&
    material.paramsF32.length >= material.paramCols * 16
  );
}

/** Window-aware facade selection, then the largest triangle's UV transform.
 * Separate roof/facade samples preserve constant Top UVs (padding knife bands).
 * This transfers a representative surface, not the source building's layout.
 */
export async function analyzeBuildingMaterial(
  payload: LotModelPayload,
): Promise<BuildingMaterialSource> {
  const roots = await parseLotModelObjects(payload.glbs);
  const bounds = new THREE.Box3();
  roots.forEach((root) => bounds.expandByObject(root));
  const roofMinHeight = (bounds.min.z + bounds.max.z) / 2;
  const groups = {
    facade: new Map<string, Sample>(),
    roof: new Map<string, Sample>(),
  };
  interface Sample {
    area: number;
    largest: number;
    surface: SurfaceMaterial;
  }
  try {
    roots.forEach((root, meshIndex) => {
      const materialIndex = payload.meshMaterialIndices[meshIndex];
      const material = payload.materials[materialIndex];
      if (
        payload.meshUvKinds[meshIndex] !== 2 ||
        !material ||
        !isBuildingMaterial(material)
      )
        return;
      root.updateMatrixWorld(true);
      root.traverse((object) => {
        if (!(object instanceof THREE.Mesh)) return;
        const geometry = object.geometry;
        const position = geometry.getAttribute("position");
        const selector = geometry.getAttribute("uv1");
        const base = geometry.getAttribute("uv2");
        const top = geometry.getAttribute("uv3");
        if (!position || !selector || !base || !top) return;
        const count = geometry.index?.count ?? position.count;
        for (let i = 0; i + 2 < count; i += 3) {
          const ids = [0, 1, 2].map(
            (j) => geometry.index?.getX(i + j) ?? i + j,
          );
          const columns = ids.map((id) =>
            Math.floor(selector.getX(id) * 255 + 0.1),
          );
          const column = columns[0];
          if (
            columns.some((value) => value !== column) ||
            column < 0 ||
            column >= material.paramCols
          )
            continue;
          const points = ids.map((id) =>
            new THREE.Vector3()
              .fromBufferAttribute(position, id)
              .applyMatrix4(object.matrixWorld),
          );
          const ab = points[1].clone().sub(points[0]);
          const ac = points[2].clone().sub(points[0]);
          const normal = ab.clone().cross(ac);
          const area = normal.length() / 2;
          if (area < 1e-7) continue;
          normal.normalize();
          const roof = normal.z > 0.7;
          // Lots may include a large upward-facing lawn/base mesh. Do not
          // choose that as the building's roof merely because it has more area.
          if (
            roof &&
            points.reduce((sum, point) => sum + point.z, 0) / 3 < roofMinHeight
          )
            continue;
          if (!roof && Math.abs(normal.z) > 0.3) continue;
          const h = roof
            ? new THREE.Vector3(1, 0, 0)
            : new THREE.Vector3(-normal.y, normal.x, 0).normalize();
          const v = roof
            ? new THREE.Vector3(0, 1, 0)
            : new THREE.Vector3(0, 0, 1);
          const x1 = ab.dot(h),
            y1 = ab.dot(v),
            x2 = ac.dot(h),
            y2 = ac.dot(v);
          const determinant = x1 * y2 - x2 * y1;
          if (Math.abs(determinant) < 1e-7) continue;
          const transform = (attribute: typeof base): SurfaceUv => {
            const a = [attribute.getX(ids[0]), attribute.getY(ids[0])];
            const b = [attribute.getX(ids[1]), attribute.getY(ids[1])];
            const c = [attribute.getX(ids[2]), attribute.getY(ids[2])];
            return {
              offset: a as [number, number],
              horizontal: a.map(
                (value, j) =>
                  ((b[j] - value) * y2 - (c[j] - value) * y1) / determinant,
              ) as [number, number],
              vertical: a.map(
                (value, j) =>
                  (x1 * (c[j] - value) - x2 * (b[j] - value)) / determinant,
              ) as [number, number],
            };
          };
          const surface: SurfaceMaterial = {
            materialIndex,
            column,
            seed: selector.getY(ids[0]),
            base: transform(base),
            top: transform(top),
            windowCoverage: 0,
            topCoverage: 0,
          };
          if (
            ![
              surface.seed,
              ...Object.values(surface.base).flat(),
              ...Object.values(surface.top).flat(),
            ].every(Number.isFinite)
          )
            continue;
          const samples = roof ? groups.roof : groups.facade;
          const key = `${materialIndex}:${column}`;
          const previous = samples.get(key);
          samples.set(key, {
            area: (previous?.area ?? 0) + area,
            largest: Math.max(previous?.largest ?? 0, area),
            surface:
              previous && previous.largest > area ? previous.surface : surface,
          });
        }
      });
    });
    const alphaMaps = new Map<number, BuildingAlphaMaps>();
    await Promise.all(
      [
        ...new Set(
          [...groups.facade.values()].map(
            ({ surface }) => surface.materialIndex,
          ),
        ),
      ].map(async (materialIndex) => {
        try {
          alphaMaps.set(
            materialIndex,
            await readBuildingAlphaMaps(payload.materials[materialIndex]),
          );
        } catch {
          // Keep the established area fallback if a package supplies a PNG
          // variant the browser cannot decode.
        }
      }),
    );
    for (const sample of groups.facade.values()) {
      const maps = alphaMaps.get(sample.surface.materialIndex);
      if (maps)
        Object.assign(
          sample.surface,
          measureFacadeCoverage(
            payload.materials[sample.surface.materialIndex],
            sample.surface,
            maps,
          ),
        );
    }
    const dominant = (samples: Map<string, Sample>, preferWindows = false) => {
      const values = [...samples.values()];
      const maxArea = Math.max(...values.map((sample) => sample.area), 0);
      const significant = values.filter(
        (sample) => sample.area >= maxArea * 0.05,
      );
      const maxCoverage = Math.max(
        ...significant.map(({ surface }) => surface.windowCoverage),
        0,
      );
      // Ignore tiny window/decal pieces. Among meaningful candidates, authored
      // window coverage wins; area remains the fallback and tie-breaker.
      const windowed = significant
        .filter(
          (sample) =>
            preferWindows &&
            sample.surface.windowCoverage >= Math.max(0.02, maxCoverage * 0.2),
        )
        .sort(
          (a, b) =>
            b.area * Math.sqrt(b.surface.windowCoverage) -
            a.area * Math.sqrt(a.surface.windowCoverage),
        );
      return (windowed[0] ?? values.sort((a, b) => b.area - a.area)[0])
        ?.surface;
    };
    const facade = dominant(groups.facade, true);
    if (!facade) throw new Error("No usable building4 facade surface");
    const wall = [...groups.facade.values()]
      .filter(
        ({ surface }) =>
          alphaMaps.has(surface.materialIndex) &&
          surface.windowCoverage < 0.005,
      )
      .sort((a, b) => b.area - a.area)[0]?.surface;
    return {
      payload,
      facade,
      roof: dominant(groups.roof) ?? wall ?? facade,
      wall,
    };
  } finally {
    roots.forEach((root) =>
      root.traverse((object) => {
        if (!(object instanceof THREE.Mesh)) return;
        object.geometry.dispose();
        const materials = Array.isArray(object.material)
          ? object.material
          : [object.material];
        materials.forEach((material) => material.dispose());
      }),
    );
  }
}
