/**
 * WPF 行主序 12 floats → TRS（纯函数，无 three 依赖）。
 *
 * 语义与 `unitGizmos.unitMatrix(THREE, transform).decompose()` 一致：
 * three 矩阵的行 = 输入的第 0/1/2 行（行向量约定转置），缩放取**列长**，
 * 四元数分支与 three `Quaternion.setFromRotationMatrix` 对齐。
 * 等价性由 `src/render/resolve/transform.test.ts` 对拍保证（25 组随机矩阵，1e-6）。
 */
import type { Quat, TransformSpec, Vec3 } from "../contract/types";

/** 行主序 3×4 → 三行基 + 平移；非法输入返回 null。 */
export function affineFromRowMajor12(
  matrix: readonly number[] | null | undefined,
): { x: Vec3; y: Vec3; z: Vec3; origin: Vec3 } | null {
  if (!matrix || matrix.length !== 12) return null;
  if (!matrix.every((value) => Number.isFinite(value))) return null;
  return {
    x: [matrix[0], matrix[3], matrix[6]],
    y: [matrix[1], matrix[4], matrix[7]],
    z: [matrix[2], matrix[5], matrix[8]],
    origin: [matrix[9], matrix[10], matrix[11]],
  };
}

/** 与 three `Matrix4.decompose` 同口径的 TRS 分解（含负行列式处理）。 */
export function rowMajor12ToTrs(
  matrix: readonly number[] | null | undefined,
): TransformSpec | null {
  const affine = affineFromRowMajor12(matrix);
  if (!affine) return null;
  const { x, y, z, origin } = affine;
  // three 矩阵 M 的行 = (x, y, z)：列长才是缩放
  const sx = Math.hypot(x[0], y[0], z[0]);
  const sy = Math.hypot(x[1], y[1], z[1]);
  const sz = Math.hypot(x[2], y[2], z[2]);
  const det =
    x[0] * (y[1] * z[2] - y[2] * z[1]) -
    x[1] * (y[0] * z[2] - y[2] * z[0]) +
    x[2] * (y[0] * z[1] - y[1] * z[0]);
  const signedSx = det < 0 ? -sx : sx;
  const scale: Vec3 = [signedSx, sy, sz];
  const ix = sx === 0 ? 0 : 1 / signedSx;
  const iy = sy === 0 ? 0 : 1 / sy;
  const iz = sz === 0 ? 0 : 1 / sz;
  const rotation: readonly [Vec3, Vec3, Vec3] = [
    [x[0] * ix, x[1] * iy, x[2] * iz],
    [y[0] * ix, y[1] * iy, y[2] * iz],
    [z[0] * ix, z[1] * iy, z[2] * iz],
  ];
  return {
    position: origin,
    quaternion: quaternionFromRotationRows(rotation),
    scale,
  };
}

/** 旋转矩阵（行主序 3×3）→ 四元数；分支与 three 的实现一致。 */
export function quaternionFromRotationRows(
  rotation: readonly [Vec3, Vec3, Vec3],
): Quat {
  const [r0, r1, r2] = rotation;
  const m11 = r0[0], m12 = r0[1], m13 = r0[2];
  const m21 = r1[0], m22 = r1[1], m23 = r1[2];
  const m31 = r2[0], m32 = r2[1], m33 = r2[2];
  const trace = m11 + m22 + m33;
  if (trace > 0) {
    const s = 0.5 / Math.sqrt(trace + 1);
    return [(m32 - m23) * s, (m13 - m31) * s, (m21 - m12) * s, 0.25 / s];
  }
  if (m11 > m22 && m11 > m33) {
    const s = 2 * Math.sqrt(1 + m11 - m22 - m33);
    return [0.25 * s, (m12 + m21) / s, (m13 + m31) / s, (m32 - m23) / s];
  }
  if (m22 > m33) {
    const s = 2 * Math.sqrt(1 + m22 - m11 - m33);
    return [(m12 + m21) / s, 0.25 * s, (m23 + m32) / s, (m13 - m31) / s];
  }
  const s = 2 * Math.sqrt(1 + m33 - m11 - m22);
  return [(m13 + m31) / s, (m23 + m32) / s, 0.25 * s, (m21 - m12) / s];
}
