/**
 * Lot 地面计划（纯函数，无 three / 无贴图 IO）。
 *
 * 对应旧 `editorGround.ts` 的几何部分（buildLotRect 的角点、placementInverse、
 * buildLotDimensions 的线段与锚点）。贴图合成/缓存仍留在旧模块，后续并入
 * `exec/three/ground` + session resource store。
 */
import type { Vec3 } from "../contract/types";
import { affineFromRowMajor12, invertAffine, type AffineRows } from "../resolve/matrix";

export interface LotGroundPlan {
  readonly width: number;
  readonly depth: number;
  /** 地面矩形四角（lot 局部，Z-up；z=0.05 为边线高度）。 */
  readonly corners: readonly [Vec3, Vec3, Vec3, Vec3];
  /** 填充面偏移（旧 buildLotRect: fill.position.z = 0.02）。 */
  readonly fillZ: number;
  /** LotPlacementTransform 的逆（无 placement = null）。 */
  readonly placementInverse: AffineRows | null;
}

export function lotGroundPlan(
  lotSize: readonly [number, number] | null | undefined,
  placement: readonly number[] | null | undefined,
): LotGroundPlan | null {
  if (!lotSize) return null;
  const [width, depth] = lotSize;
  if (!(width > 0) || !(depth > 0)) return null;
  const halfWidth = width / 2;
  const halfDepth = depth / 2;
  const affine = placement ? affineFromRowMajor12(placement) : null;
  return {
    width,
    depth,
    corners: [
      [-halfWidth, -halfDepth, 0.05],
      [halfWidth, -halfDepth, 0.05],
      [halfWidth, halfDepth, 0.05],
      [-halfWidth, halfDepth, 0.05],
    ],
    fillZ: 0.02,
    placementInverse: affine ? invertAffine(affine) : null,
  };
}

export interface DimensionAnchor {
  readonly axis: "length" | "width" | "height";
  /** 线段中点（lot 局部，Z-up）；调用方按地面矩阵换算到游戏系投影。 */
  readonly point: Vec3;
  readonly text: string;
}

export interface LotDimensionPlan {
  readonly points: readonly Vec3[];
  readonly anchors: readonly DimensionAnchor[];
}

/** 尺度标注线段与锚点；与旧 buildLotDimensions 的 118–168 行同口径。 */
export function lotDimensionPlan(
  lotSize: readonly [number, number] | null | undefined,
  height: number,
): LotDimensionPlan | null {
  if (!lotSize) return null;
  const [w, d] = lotSize;
  if (!(w > 0) || !(d > 0)) return null;
  const offset = Math.min(Math.max(Math.min(w, d) * 0.08, 0.5), 4);
  const z = 0.15;
  const x0 = -w / 2;
  const x1 = w / 2;
  const y0 = -d / 2;
  const y1 = d / 2;
  const xa = x0 - offset;
  const ya = y0 - offset;
  const tick = offset * 0.35;
  const points: Vec3[] = [
    [x0, ya, z], [x1, ya, z],
    [x0, y0, z], [x0, ya - tick, z],
    [x1, y0, z], [x1, ya - tick, z],
    [xa, y0, z], [xa, y1, z],
    [x0, y0, z], [xa - tick, y0, z],
    [x0, y1, z], [xa - tick, y1, z],
  ];
  const hasHeight = height > 0.01;
  if (hasHeight) {
    points.push(
      [xa, ya, z], [xa, ya, height],
      [xa - tick, ya, z], [xa + tick, ya, z],
      [xa - tick, ya, height], [xa + tick, ya, height],
    );
  }
  const anchors: DimensionAnchor[] = [
    { axis: "length", point: [0, ya, z], text: `${w.toFixed(1)} m` },
    { axis: "width", point: [xa, 0, z], text: `${d.toFixed(1)} m` },
  ];
  if (hasHeight) {
    anchors.push({
      axis: "height",
      point: [xa, ya, height / 2],
      text: `${height.toFixed(1)} m`,
    });
  }
  return { points, anchors };
}
