import { Box3, BufferGeometry, Matrix4, Mesh, Vector3 } from "three";
import { DecalGeometry } from "three/examples/jsm/geometries/DecalGeometry.js";
import type { Euler } from "three";

type Node = { box: Box3; triangles?: number[]; left?: Node; right?: Node };
type Index = {
  position: unknown;
  index: unknown;
  positionVersion: number;
  indexVersion: number;
  normal: unknown;
  normalVersion: number;
  root: Node | null;
  projections: Map<string, BufferGeometry>;
};

/** Per-viewport ownership: geometry identity and buffer versions invalidate receiver indices. */
export function createDecalReceiverIndex() {
  let entries = new WeakMap<BufferGeometry, Index>();
  function getRoot(geometry: BufferGeometry): Node | null {
    const position = geometry.getAttribute("position");
    const index = geometry.index;
    if (!position) return null;
    const version =
      "version" in position ? position.version : position.data.version;
    const normal = geometry.getAttribute("normal");
    const normalVersion = normal
      ? "version" in normal
        ? normal.version
        : normal.data.version
      : 0;
    const old = entries.get(geometry);
    if (
      old &&
      old.position === position &&
      old.index === index &&
      old.positionVersion === version &&
      old.indexVersion === (index?.version ?? 0) &&
      old.normal === normal &&
      old.normalVersion === normalVersion
    )
      return old.root;
    const boxes: Box3[] = [];
    let valid = true;
    // Preserve the old rejection of a receiver containing any non-finite vertex.
    for (let i = 0; i < position.count; i++) {
      if (
        ![position.getX(i), position.getY(i), position.getZ(i)].every(
          Number.isFinite,
        )
      ) {
        valid = false;
        break;
      }
    }
    const count = index?.count ?? position.count;
    if (valid)
      for (let offset = 0; offset + 2 < count; offset += 3) {
        const box = new Box3();
        for (let k = 0; k < 3; k++) {
          const vertex = index ? index.getX(offset + k) : offset + k;
          box.expandByPoint(
            new Vector3().fromBufferAttribute(position, vertex),
          );
        }
        boxes.push(box);
      }
    function build(ids: number[]): Node {
      const box = new Box3();
      for (const id of ids) box.union(boxes[id]);
      if (ids.length <= 32) return { box, triangles: ids };
      const size = box.getSize(new Vector3());
      const axis =
        size.x >= size.y && size.x >= size.z
          ? "x"
          : size.y >= size.z
            ? "y"
            : "z";
      ids.sort(
        (a, b) =>
          boxes[a].min[axis] +
          boxes[a].max[axis] -
          (boxes[b].min[axis] + boxes[b].max[axis]),
      );
      const middle = ids.length >>> 1;
      return {
        box,
        left: build(ids.slice(0, middle)),
        right: build(ids.slice(middle)),
      };
    }
    const root = boxes.length ? build(boxes.map((_, i) => i)) : null;
    entries.set(geometry, {
      position,
      index,
      positionVersion: version,
      indexVersion: index?.version ?? 0,
      normal,
      normalVersion,
      root,
      projections: new Map(),
    });
    return root;
  }
  return {
    clear() {
      entries = new WeakMap();
    },
    project(
      mesh: Mesh,
      position: Vector3,
      orientation: Euler,
      size: Vector3,
    ): BufferGeometry | null {
      const receiver = this.select(mesh, position, orientation, size);
      if (!receiver) return null;
      const entry = entries.get(mesh.geometry)!;
      const key = [
        ...mesh.matrixWorld.elements,
        ...position.toArray(),
        orientation.x,
        orientation.y,
        orientation.z,
        orientation.order,
        ...size.toArray(),
      ].join(",");
      const cached = entry.projections.get(key);
      if (cached) {
        receiver.geometry.dispose();
        return cached.clone();
      }
      const projected = new DecalGeometry(
        receiver,
        position,
        orientation,
        size,
      );
      receiver.geometry.dispose();
      // CPU-only templates, bounded per receiver. Each caller owns its returned geometry.
      if (entry.projections.size >= 16)
        entry.projections.delete(entry.projections.keys().next().value!);
      entry.projections.set(key, projected.clone());
      return projected;
    },
    /** Conservative broad phase only; Three retains all exact clipping and interpolation. */
    select(
      mesh: Mesh,
      position: Vector3,
      orientation: Euler,
      size: Vector3,
    ): Mesh | null {
      const root = getRoot(mesh.geometry);
      if (!root) return null;
      const transform = new Matrix4()
        .makeRotationFromEuler(orientation)
        .setPosition(position);
      transform.premultiply(mesh.matrixWorld.clone().invert());
      const half = size.clone().multiplyScalar(0.5);
      const query = new Box3(half.clone().negate(), half).applyMatrix4(
        transform,
      );
      // Roundoff at transformed box boundaries must never reject touching faces.
      query.expandByScalar(
        Math.max(1, query.min.length(), query.max.length()) * 1e-10,
      );
      const triangles: number[] = [];
      function visit(node: Node) {
        if (!node.box.intersectsBox(query)) return;
        if (node.triangles) triangles.push(...node.triangles);
        else {
          visit(node.left!);
          visit(node.right!);
        }
      }
      visit(root);
      if (!triangles.length) return null;
      // Original face order preserves exact output ordering of DecalGeometry.
      triangles.sort((a, b) => a - b);
      const indices: number[] = [];
      for (const triangle of triangles)
        for (let k = 0; k < 3; k++) {
          const offset = triangle * 3 + k;
          indices.push(
            mesh.geometry.index ? mesh.geometry.index.getX(offset) : offset,
          );
        }
      const geometry = new BufferGeometry();
      geometry.setAttribute("position", mesh.geometry.getAttribute("position"));
      const normal = mesh.geometry.getAttribute("normal");
      if (normal) geometry.setAttribute("normal", normal);
      geometry.setIndex(indices);
      const receiver = new Mesh(geometry);
      receiver.matrixWorld.copy(mesh.matrixWorld);
      return receiver;
    },
  };
}
