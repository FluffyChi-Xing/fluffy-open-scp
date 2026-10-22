import { Vector3 } from "three";
import type { BuildingPart, Vec3 } from "./schema";

/** Convex boundary polygons. Boolean operations happen before UV projection,
 * so splits cannot introduce material seams or stretch an atlas over a cavity. */
export interface SolidFace {
  vertices: Vector3[];
  normal: Vector3;
}
const EPS = 1e-5;

function face(vertices: Vector3[]): SolidFace | null {
  const clean = vertices.filter(
    (p, i) =>
      p.distanceToSquared(vertices[(i + 1) % vertices.length]) > EPS ** 2,
  );
  if (clean.length < 3) return null;
  for (let i = 1; i < clean.length - 1; i++) {
    const normal = clean[i]
      .clone()
      .sub(clean[0])
      .cross(clean[i + 1].clone().sub(clean[0]));
    if (normal.lengthSq() > EPS ** 4)
      return { vertices: clean, normal: normal.normalize() };
  }
  return null;
}

class Plane {
  constructor(
    public normal: Vector3,
    public distance: number,
  ) {}
  split(
    p: SolidFace,
    coplanarFront: SolidFace[],
    coplanarBack: SolidFace[],
    front: SolidFace[],
    back: SolidFace[],
  ) {
    const ds = p.vertices.map((v) => this.normal.dot(v) - this.distance);
    const hasFront = ds.some((d) => d > EPS),
      hasBack = ds.some((d) => d < -EPS);
    if (!hasFront && !hasBack) {
      (this.normal.dot(p.normal) > 0 ? coplanarFront : coplanarBack).push(p);
    } else if (!hasBack) front.push(p);
    else if (!hasFront) back.push(p);
    else {
      const f: Vector3[] = [],
        b: Vector3[] = [];
      p.vertices.forEach((v, i) => {
        const j = (i + 1) % p.vertices.length,
          d = ds[i],
          e = ds[j];
        if (d >= -EPS) f.push(v);
        if (d <= EPS) b.push(v);
        if ((d > EPS && e < -EPS) || (d < -EPS && e > EPS)) {
          const intersection = v.clone().lerp(p.vertices[j], d / (d - e));
          f.push(intersection);
          b.push(intersection);
        }
      });
      const pf = face(f),
        pb = face(b);
      if (pf) front.push(pf);
      if (pb) back.push(pb);
    }
  }
}

/** BSP boundary clipping for a small number of coarse solids, not a voxel grid.
 * Coplanar faces go to the front/back according to winding, eliminating shared
 * walls and duplicate roof planes at touching or overlapping blocks. */
class BoundaryTree {
  plane?: Plane;
  polygons: SolidFace[] = [];
  front?: BoundaryTree;
  back?: BoundaryTree;
  constructor(polygons: SolidFace[] = []) {
    this.build(polygons);
  }
  build(polygons: SolidFace[]) {
    if (!polygons.length) return;
    this.plane ??= new Plane(
      polygons[0].normal.clone(),
      polygons[0].normal.dot(polygons[0].vertices[0]),
    );
    const f: SolidFace[] = [],
      b: SolidFace[] = [];
    for (const p of polygons)
      this.plane.split(p, this.polygons, this.polygons, f, b);
    if (f.length) (this.front ??= new BoundaryTree()).build(f);
    if (b.length) (this.back ??= new BoundaryTree()).build(b);
  }
  all(): SolidFace[] {
    return [
      ...this.polygons,
      ...(this.front?.all() ?? []),
      ...(this.back?.all() ?? []),
    ];
  }
  invert() {
    this.polygons = this.polygons.map((p) => ({
      vertices: [...p.vertices].reverse(),
      normal: p.normal.clone().negate(),
    }));
    if (this.plane) {
      this.plane.normal.negate();
      this.plane.distance *= -1;
    }
    this.front?.invert();
    this.back?.invert();
    [this.front, this.back] = [this.back, this.front];
  }
  clip(polygons: SolidFace[]): SolidFace[] {
    if (!this.plane) return polygons.slice();
    let f: SolidFace[] = [],
      b: SolidFace[] = [];
    for (const p of polygons) this.plane.split(p, f, b, f, b);
    if (this.front) f = this.front.clip(f);
    b = this.back ? this.back.clip(b) : [];
    return f.concat(b);
  }
  clipTo(other: BoundaryTree) {
    this.polygons = other.clip(this.polygons);
    this.front?.clipTo(other);
    this.back?.clipTo(other);
  }
}

function boolean(
  a: SolidFace[],
  b: SolidFace[],
  subtract: boolean,
): SolidFace[] {
  if (!a.length) return subtract ? [] : b;
  if (!b.length) return a;
  const left = new BoundaryTree(a),
    right = new BoundaryTree(b);
  if (subtract) left.invert();
  left.clipTo(right);
  right.clipTo(left);
  right.invert();
  right.clipTo(left);
  right.invert();
  left.build(right.all());
  if (subtract) left.invert();
  return left.all();
}

function boxFaces(
  size: Vec3,
  localCenter: Vec3,
  part: BuildingPart,
): SolidFace[] {
  const points: Vector3[] = [];
  const angle = part.rotation ?? 0,
    c = Math.cos(angle),
    s = Math.sin(angle);
  for (const z of [-1, 1])
    for (const y of [-1, 1])
      for (const x of [-1, 1]) {
        const lx = localCenter[0] + (x * size[0]) / 2,
          ly = localCenter[1] + (y * size[1]) / 2;
        points.push(
          new Vector3(
            part.center[0] + lx * c - ly * s,
            part.center[1] + lx * s + ly * c,
            part.center[2] + localCenter[2] + (z * size[2]) / 2,
          ),
        );
      }
  return [
    [0, 2, 3, 1],
    [4, 5, 7, 6],
    [0, 1, 5, 4],
    [2, 6, 7, 3],
    [0, 4, 6, 2],
    [1, 3, 7, 5],
  ].map((ids) => face(ids.map((i) => points[i]))!);
}

export function buildMassFaces(parts: readonly BuildingPart[]): SolidFace[] {
  let boundary: SolidFace[] = [];
  for (const part of parts.filter((p) => p.kind === "mass")) {
    let solid = boxFaces(part.size, [0, 0, 0], part);
    for (const hole of part.cutouts ?? [])
      solid = boolean(solid, boxFaces(hole.size, hole.center, part), true);
    boundary = boolean(boundary, solid, false);
  }
  return boundary;
}
