/**
 * 仿射矩阵纯函数（无 three）。
 *
 * 两种读法在仓库里长期并存，这里显式区分、不再混用：
 * - `AffineRows`：WPF 行主序 3×4，行 = 基向量（unit/placement 口径）；
 * - `decalFramePlan` 的轴：three 矩阵的**列**（m0,m1,m2 / m3,m4,m5 / m6,m7,m8）。
 */
import type { Vec3 } from "../contract/types";

export interface AffineRows {
  readonly x: Vec3;
  readonly y: Vec3;
  readonly z: Vec3;
  readonly origin: Vec3;
}

export function affineFromRowMajor12(
  matrix: readonly number[] | null | undefined,
): AffineRows | null {
  if (!matrix || matrix.length !== 12) return null;
  if (!matrix.every((value) => Number.isFinite(value))) return null;
  return {
    x: [matrix[0], matrix[3], matrix[6]],
    y: [matrix[1], matrix[4], matrix[7]],
    z: [matrix[2], matrix[5], matrix[8]],
    origin: [matrix[9], matrix[10], matrix[11]],
  };
}

/** 行主序仿射逆（与 three `Matrix4.set(...).invert()` 等价，逐元素 1e-6）。 */
export function invertAffine(affine: AffineRows): AffineRows | null {
  const [r0, r1, r2] = [affine.x, affine.y, affine.z];
  // M = [[r00,r10,r20],[r01,r11,r21],[r02,r12,r22]] = Rᵀ；先求 M⁻¹
  const c0 = r0[0], c1 = r1[0], c2 = r2[0];
  const c3 = r0[1], c4 = r1[1], c5 = r2[1];
  const c6 = r0[2], c7 = r1[2], c8 = r2[2];
  const det =
    c0 * (c4 * c8 - c5 * c7) -
    c1 * (c3 * c8 - c5 * c6) +
    c2 * (c3 * c7 - c4 * c6);
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) return null;
  const inv = 1 / det;
  const i00 = (c4 * c8 - c5 * c7) * inv;
  const i01 = (c2 * c7 - c1 * c8) * inv;
  const i02 = (c1 * c5 - c2 * c4) * inv;
  const i10 = (c5 * c6 - c3 * c8) * inv;
  const i11 = (c0 * c8 - c2 * c6) * inv;
  const i12 = (c2 * c3 - c0 * c5) * inv;
  const i20 = (c3 * c7 - c4 * c6) * inv;
  const i21 = (c1 * c6 - c0 * c7) * inv;
  const i22 = (c0 * c4 - c1 * c3) * inv;
  const rowX: Vec3 = [i00, i10, i20];
  const rowY: Vec3 = [i01, i11, i21];
  const rowZ: Vec3 = [i02, i12, i22];
  const t = affine.origin;
  const translate = (row: Vec3) =>
    -(row[0] * t[0] + row[1] * t[1] + row[2] * t[2]);
  return {
    x: rowX,
    y: rowY,
    z: rowZ,
    origin: [translate(rowX), translate(rowY), translate(rowZ)],
  };
}

/** 行主序 → three 列主序 elements[16]。 */
export function affineToColumnMajor16(affine: AffineRows): number[] {
  return [
    affine.x[0], affine.y[0], affine.z[0], 0,
    affine.x[1], affine.y[1], affine.z[1], 0,
    affine.x[2], affine.y[2], affine.z[2], 0,
    affine.origin[0], affine.origin[1], affine.origin[2], 1,
  ];
}
