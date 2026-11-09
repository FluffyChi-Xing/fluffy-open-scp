/**
 * Lot 地面计划（纯函数，无 three / 无贴图 IO）。
 *
 * 对应旧 `editorGround.ts` 的几何部分（buildLotRect 的角点、placementInverse、
 * buildLotDimensions 的线段与锚点）。贴图合成/缓存仍留在旧模块，后续并入
 * `exec/three/ground` + session resource store。
 */
import type { Vec3 } from "../contract/types";
import { affineFromRowMajor12, invertAffine, type AffineRows } from "../resolve/matrix";

/** 临街箭头边角：0=+Y / 90=−X / 180=−Y / 270=+X（相对 +Y 逆时针）。 */
export type StreetFacingDegrees = 0 | 90 | 180 | 270;

/** 与 Building40 `rotate_xyz_matrix` / `rotate_point` 同口径。 */
function rotateYawXY(
  x: number,
  y: number,
  degrees: StreetFacingDegrees,
): [number, number] {
  switch (degrees) {
    case 0:
      return [x, y];
    case 90:
      return [-y, x];
    case 180:
      return [-x, -y];
    case 270:
      return [y, -x];
  }
}

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

export interface StreetSidePlan {
  /** 三角化后的箭头轮廓顶点（lot 局部，Z-up）。 */
  readonly triangles: readonly Vec3[];
  /** 箭尖（lot 局部）；供调试/后续标签使用。 */
  readonly tip: Vec3;
  /** 相对 +Y 的边角（0=+Y / 90=−X / 180=−Y / 270=+X）。 */
  readonly facingDegrees: StreetFacingDegrees;
}

/**
 * 临街侧箭头：游戏 unit-lot 本地系约定 **+Y = 前缘 = 临街侧**
 * （`docs/re/dev-dump-lot-placement-and-script-resources.md` §2.3；
 * `docs/re/blender-white-model-six-slot-pipeline-20261023.md` §2.2）。
 * 模板 straddles +Y 边；`facingDegrees` 用与 Building40 相同的 yaw 旋到所选边。
 */
export function streetSidePlan(
  lotSize: readonly [number, number] | null | undefined,
  facingDegrees: StreetFacingDegrees = 0,
): StreetSidePlan | null {
  if (!lotSize) return null;
  const [width, depth] = lotSize;
  if (!(width > 0) || !(depth > 0)) return null;
  const edge = depth / 2;
  const size = Math.min(Math.max(Math.min(width, depth) * 0.12, 2), 12);
  const z = 0.22;
  const shaftHalf = size * 0.14;
  const headHalf = size * 0.3;
  const base = edge - size * 0.55;
  const shoulder = edge + size * 0.1;
  const rotate = (x: number, y: number): Vec3 => {
    const [rx, ry] = rotateYawXY(x, y, facingDegrees);
    return [rx, ry, z];
  };
  const tip = rotate(0, edge + size * 0.75);
  const triangles: Vec3[] = [
    rotate(-shaftHalf, base), rotate(shaftHalf, base), rotate(shaftHalf, shoulder),
    rotate(shaftHalf, shoulder), rotate(-shaftHalf, shoulder), rotate(-shaftHalf, base),
    rotate(-headHalf, shoulder), rotate(headHalf, shoulder), tip,
  ];
  return { triangles, tip, facingDegrees };
}

export interface LotDimensionPlan {
  readonly points: readonly Vec3[];
  readonly anchors: readonly DimensionAnchor[];
  /** 临街侧箭头；null = 无 lotSize/非法尺寸。 */
  readonly streetSide: StreetSidePlan | null;
}

/** 尺度标注线段与锚点；与旧 buildLotDimensions 的 118–168 行同口径。 */
export function lotDimensionPlan(
  lotSize: readonly [number, number] | null | undefined,
  height: number,
  streetFacingDegrees: StreetFacingDegrees = 0,
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
  return {
    points,
    anchors,
    streetSide: streetSidePlan(lotSize, streetFacingDegrees),
  };
}
