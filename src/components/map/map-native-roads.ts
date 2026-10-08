import * as THREE from "three";
import type { Region3DData, StateSource } from "@/lib/region-map";
import { nativeGeometry, nativeTexture } from "./map-native-assets";
import { sweepSections } from "./map-sweep";

function inside(x: number, y: number, b: readonly number[]): boolean {
  return x >= b[0] && y >= b[1] && x < b[2] && y < b[3];
}
/** Empty-map preview uses only region zero, including roads inside city bounds. */
export function ownsRoadPoint(source: StateSource): boolean {
  return source.site === "0";
}
/** Source splines and ribbon width/UV/offsets, never raster-derived centerlines. */
export function createNativeRoads(
  data: Region3DData,
  pixels: Record<string, ImageData>,
  height: (x: number, y: number) => number,
): THREE.Group {
  const group = new THREE.Group();
  group.name = "regional-road-network";
  const batches = new Map<
    number,
    { positions: number[]; uvs: number[]; indices: number[] }
  >();
  const sources = (data.saveLayers?.sources ?? []).filter(ownsRoadPoint);
  for (const source of sources)
    for (const record of source.curves) {
      const components = data.roadAssets?.ribbons[record.entryId] ?? [];
      const controls = record.controls.map(
        (p) =>
          new THREE.Vector3(
            p[0] + source.origin[0],
            p[1] + source.origin[1],
            p[2],
          ),
      );
      const curve = new THREE.CubicBezierCurve3(
        controls[0],
        controls[1],
        controls[2],
        controls[3],
      );
      const length = curve.getLength(),
        steps = Math.max(1, Math.ceil(length / 8));
      for (const component of components) {
        if (!pixels[component.texture]) continue;
        const batch = batches.get(component.texture) ?? {
          positions: [],
          uvs: [],
          indices: [],
        };
        batches.set(component.texture, batch);
        const width = component.worldSize[0] * component.scale[0];
        const period = Math.max(
          0.01,
          // GenerateExtrusionRibbon repeats by worldSize.y, without scale.y.
          // Rail components use scale.y=21; applying it here stretches sleepers.
          Math.abs(component.worldSize[1]),
        );
        for (let j = 0; j < steps; j++) {
          const mid = curve.getPointAt((j + 0.5) / steps);
          const span = data.size * data.metersPerPixel;
          if (
            !inside(mid.x, mid.y, [
              ...data.originWorld,
              data.originWorld[0] + span,
              data.originWorld[1] + span,
            ])
          )
            continue;
          const base = batch.positions.length / 3;
          for (let end = 0; end < 2; end++) {
            const t = (j + end) / steps,
              p = curve.getPointAt(t),
              tangent = curve.getTangentAt(t);
            const norm = Math.hypot(tangent.x, tangent.y) || 1;
            const nx = tangent.y / norm,
              ny = -tangent.x / norm;
            for (let side = 0; side < 2; side++) {
              const offset = component.offset[0] + (side - 0.5) * width;
              batch.positions.push(
                p.x + nx * offset,
                p.z + component.offset[2] + 0.12,
                p.y + ny * offset,
              );
              batch.uvs.push(
                side ? component.uvEnd[0] : component.uvStart[0],
                component.uvStart[1] - (length * t) / period,
              );
            }
          }
          batch.indices.push(
            base,
            base + 2,
            base + 1,
            base + 1,
            base + 2,
            base + 3,
          );
        }
      }
    }
  for (const [id, b] of batches) {
    if (!b.positions.length) continue;
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(b.positions, 3),
    );
    geometry.setAttribute("uv", new THREE.Float32BufferAttribute(b.uvs, 2));
    geometry.setIndex(b.indices);
    geometry.computeVertexNormals();
    const map = nativeTexture(pixels[id]);
    const material = new THREE.MeshLambertMaterial({
      map,
      alphaTest: 0.75,
      side: THREE.DoubleSide,
      polygonOffset: true,
      polygonOffsetFactor: -1,
      polygonOffsetUnits: -1,
    });
    material.addEventListener("dispose", () => map.dispose());
    group.add(new THREE.Mesh(geometry, material));
  }
  appendModelSweeps(group, data, pixels, height);
  appendRegionalJunctions(group, data, pixels);
  return group;
}

function appendRegionalJunctions(
  group: THREE.Group,
  data: Region3DData,
  pixels: Record<string, ImageData>,
) {
  for (const instance of data.roadAssets?.instances ?? []) {
    const model = data.roadAssets?.models[instance.model];
    if (!model || !pixels[`model:${instance.model}`]) continue;
    const [x, y, z] = instance.position,
      r = instance.rotation,
      s = instance.scale;
    const span = data.size * data.metersPerPixel;
    if (
      !inside(x, y, [
        ...data.originWorld,
        data.originWorld[0] + span,
        data.originWorld[1] + span,
      ])
    )
      continue;
    // cSPTransform stores x/y/z axis vectors consecutively. Pack game z-up
    // into the shared map root's (x,height,y); that root owns the reflection.
    const transform = new THREE.Matrix4().set(
      r[0] * s,
      r[3] * s,
      r[6] * s,
      x,
      r[2] * s,
      r[5] * s,
      r[8] * s,
      z,
      r[1] * s,
      r[4] * s,
      r[7] * s,
      y,
      0,
      0,
      0,
      1,
    );
    const geometry = nativeGeometry(model);
    geometry.applyMatrix4(transform);
    const index = geometry.index!;
    // Packing z-up vertices into (x,z,y) reverses winding before the root transform.
    for (let i = 0; i < index.count; i += 3) {
      const b = index.getX(i + 1);
      index.setX(i + 1, index.getX(i + 2));
      index.setX(i + 2, b);
    }
    const map = nativeTexture(pixels[`model:${instance.model}`]);
    const material = new THREE.MeshLambertMaterial({
      map,
      side: THREE.DoubleSide,
      alphaTest: 0.5,
    });
    material.addEventListener("dispose", () => map.dispose());
    const mesh = new THREE.Mesh(geometry, material);
    mesh.name = `regional-junction-${instance.property.toString(16)}`;
    group.add(mesh);
  }
}

function appendModelSweeps(
  group: THREE.Group,
  data: Region3DData,
  pixels: Record<string, ImageData>,
  height: (x: number, y: number) => number,
) {
  const sources = (data.saveLayers?.sources ?? []).filter(ownsRoadPoint);
  const batches = new Map<
    number,
    { positions: number[]; uvs: number[]; indices: number[] }
  >();
  for (const source of sources) {
    const paths = new Map<number, typeof source.curves>();
    for (const c of source.curves) {
      const list = paths.get(c.path) ?? [];
      list.push(c);
      paths.set(c.path, list);
    }
    for (const records of paths.values()) {
      const sweeps = data.roadAssets?.sweeps[records[0].entryId] ?? [];
      if (!sweeps.length) continue;
      const remaining = [...records];
      const equal = (a: number[], b: number[]) => a.every((v, i) => v === b[i]);
      let first = remaining.findIndex(
        (r) =>
          !remaining.some(
            (o) => o !== r && equal(o.controls[3], r.controls[0]),
          ),
      );
      if (first < 0) first = 0;
      const ordered = [remaining.splice(first, 1)[0]];
      while (remaining.length) {
        const i = remaining.findIndex((r) =>
          equal(ordered.at(-1)!.controls[3], r.controls[0]),
        );
        if (i < 0) break;
        ordered.push(remaining.splice(i, 1)[0]);
      }
      if (remaining.length) continue; // Never connect disjoint topology with an invented segment.
      const path = new THREE.CurvePath<THREE.Vector3>();
      for (const r of ordered) {
        const c = r.controls.map(
          (p) =>
            new THREE.Vector3(
              p[0] + source.origin[0],
              p[1] + source.origin[1],
              p[2],
            ),
        );
        path.add(new THREE.CubicBezierCurve3(c[0], c[1], c[2], c[3]));
      }
      const length = path.getLength();
      if (length < 0.01) continue;
      for (const sweep of sweeps) {
        const model = data.roadAssets?.models[sweep.model];
        if (!model || !pixels[`model:${sweep.model}`]) continue;
        const rotation = new THREE.Euler(
          ...(sweep.rotation.map((v) => (v * Math.PI) / 180) as [
            number,
            number,
            number,
          ]),
          "ZYX",
        );
        const transform = new THREE.Matrix4().compose(
          new THREE.Vector3(),
          new THREE.Quaternion().setFromEuler(rotation),
          // GenerateExtrusionPropInstance applies mScale.x as a uniform scale.
          sweep.instance
            ? new THREE.Vector3().setScalar(sweep.scale[0])
            : new THREE.Vector3(...sweep.scale),
        );
        const vertices: THREE.Vector3[] = [];
        for (let i = 0; i < model.positions.length; i += 3)
          vertices.push(
            new THREE.Vector3(
              ...(model.positions.slice(i, i + 3) as [number, number, number]),
            ).applyMatrix4(transform),
          );
        const minY = Math.min(...vertices.map((v) => v.y)),
          maxY = Math.max(...vertices.map((v) => v.y));
        const minZ = Math.min(...vertices.map((v) => v.z)),
          maxZ = Math.max(...vertices.map((v) => v.z));
        const tileLength = Math.max(0.001, maxY - minY);
        const batch = batches.get(sweep.model) ?? {
          positions: [],
          uvs: [],
          indices: [],
        };
        batches.set(sweep.model, batch);
        for (const section of sweepSections(sweep, length, tileLength)) {
          const distance = section.start;
          const center = path.getPointAt(
            Math.min(1, (distance + section.length / 2) / length),
          );
          const span = data.size * data.metersPerPixel;
          if (
            !inside(center.x, center.y, [
              ...data.originWorld,
              data.originWorld[0] + span,
              data.originWorld[1] + span,
            ])
          )
            continue;
          const stackHeight = Math.max(0.1, maxZ - minZ);
          const copies =
            sweep.stackToGround && !sweep.relativeToGround
              ? Math.min(
                  100,
                  Math.max(
                    1,
                    Math.ceil(
                      (center.z +
                        sweep.offset[2] +
                        minZ -
                        height(center.x, center.y)) /
                        stackHeight,
                    ),
                  ),
                )
              : 1;
          for (let stack = 0; stack < copies; stack++) {
            const base = batch.positions.length / 3;
            for (const [vi, v] of vertices.entries()) {
              const rigid = sweep.instance || !sweep.distort;
              const along = rigid
                ? distance
                : distance +
                  ((v.y - minY + sweep.offset[1]) / tileLength) *
                    section.length;
              const t = Math.max(0, Math.min(1, along / length)),
                p = path.getPointAt(t),
                tangent = path.getTangentAt(t);
              const norm = Math.hypot(tangent.x, tangent.y) || 1,
                nx = tangent.y / norm,
                ny = -tangent.x / norm;
              const x =
                p.x +
                nx * (v.x + sweep.offset[0]) +
                (rigid ? tangent.x * (v.y + sweep.offset[1]) : 0);
              const y =
                p.y +
                ny * (v.x + sweep.offset[0]) +
                (rigid ? tangent.y * (v.y + sweep.offset[1]) : 0);
              const z =
                (sweep.relativeToGround ? height(x, y) : p.z) +
                v.z +
                sweep.offset[2] -
                stack * stackHeight;
              batch.positions.push(x, z, y);
              batch.uvs.push(
                model.uvs[vi * 2],
                sweep.repeatUv && !sweep.instance
                  ? along / tileLength
                  : model.uvs[vi * 2 + 1],
              );
            }
            const reflected = sweep.instance
              ? sweep.scale[0] < 0
              : sweep.scale.reduce((a, b) => a * b, 1) < 0;
            for (let i = 0; i < model.indices.length; i += 3) {
              batch.indices.push(
                base + model.indices[i],
                base + model.indices[i + (reflected ? 1 : 2)],
                base + model.indices[i + (reflected ? 2 : 1)],
              );
            }
          }
        }
      }
    }
  }
  for (const [id, b] of batches) {
    if (!b.positions.length) continue;
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(b.positions, 3),
    );
    geometry.setAttribute("uv", new THREE.Float32BufferAttribute(b.uvs, 2));
    geometry.setIndex(b.indices);
    geometry.computeVertexNormals();
    const map = nativeTexture(pixels[`model:${id}`]);
    const material = new THREE.MeshLambertMaterial({
      map,
      alphaTest: 0.5,
      side: THREE.DoubleSide,
    });
    material.addEventListener("dispose", () => map.dispose());
    group.add(new THREE.Mesh(geometry, material));
  }
}
