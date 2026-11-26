import type { LotModelPayload } from "@/api/tauri";
import { Matrix4, Quaternion, Vector3, type Mesh, type Object3D } from "three";
import { rowMajor12ToTrs } from "@/render/resolve/transform";

export interface LotModelProjection {
  payload: LotModelPayload;
  origin: readonly [number, number, number] | null;
  transform: readonly number[] | null;
  center: readonly [number, number] | null;
}

export function drawLotProjection(
  context: CanvasRenderingContext2D,
  roots: readonly Object3D[],
  projection: Omit<LotModelProjection, "payload">,
  size: readonly [number, number],
  width: number,
  height: number,
) {
  const trs = rowMajor12ToTrs(projection.transform);
  const placement = trs ? new Matrix4().compose(
    new Vector3(...trs.position), new Quaternion(...trs.quaternion), new Vector3(...trs.scale),
  ) : new Matrix4();
  const origin = new Matrix4().makeTranslation(...(projection.origin ?? [0, 0, 0]));
  const model = placement.multiply(origin);
  const [cx, cy] = projection.center ?? [0, 0];
  const point = new Vector3();

  context.clearRect(0, 0, width, height);
  context.beginPath();
  for (const root of roots) {
    root.updateMatrixWorld(true);
    root.traverse((object) => {
      const mesh = object as Mesh;
      if (!mesh.isMesh) return;
      const { index, attributes } = mesh.geometry;
      const positions = attributes.position;
      if (!positions) return;
      const world = new Matrix4().multiplyMatrices(model, mesh.matrixWorld);
      const xy = new Float32Array(positions.count * 2);
      for (let i = 0; i < positions.count; i++) {
        point.fromBufferAttribute(positions, i).applyMatrix4(world);
        xy[2 * i] = width * (0.5 + (point.x - cx) / size[0]);
        xy[2 * i + 1] = height * (0.5 + (point.y - cy) / size[1]);
      }
      for (let i = 0, count = index?.count ?? positions.count; i + 2 < count; i += 3) {
        const a = 2 * (index ? index.getX(i) : i);
        const b = 2 * (index ? index.getX(i + 1) : i + 1);
        const c = 2 * (index ? index.getX(i + 2) : i + 2);
        const area = (xy[b] - xy[a]) * (xy[c + 1] - xy[a + 1])
          - (xy[b + 1] - xy[a + 1]) * (xy[c] - xy[a]);
        if (!area) continue;
        // Match winding so overlapping triangles form a single silhouette.
        const second = area > 0 ? b : c;
        const third = area > 0 ? c : b;
        context.moveTo(xy[a], xy[a + 1]);
        context.lineTo(xy[second], xy[second + 1]);
        context.lineTo(xy[third], xy[third + 1]);
        context.closePath();
      }
    });
  }
  context.fillStyle = "rgba(255, 143, 64, 0.42)";
  context.fill();
}
