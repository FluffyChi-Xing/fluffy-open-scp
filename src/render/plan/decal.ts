/**
 * 贴花计划（纯函数）：路由 + frame 几何 + 盒深，口径与旧实现逐值一致。
 *
 * - 路由：`decalRouting.ts:4`（materialInstance u32）；
 * - frame：`lib/decalProject.ts:83`（WPF 行主序 12 floats，轴取 three 矩阵的列）；
 * - 盒深：`depth × scale`（引擎体积盒语义，见 decalProject 注释）。
 */
import type { Vec3 } from "../contract/types";

export type DecalFamily = "sdf" | "sign" | "hole" | "clip";
export type DecalSurface = "plane" | "projected";

export interface DecalRoutePlan {
  readonly family: DecalFamily;
  readonly surface: DecalSurface;
}

export interface DecalFramePlan {
  readonly origin: Vec3;
  readonly axisX: Vec3;
  readonly axisY: Vec3;
  readonly axisZ: Vec3;
  /** 可见窗全宽/全高（scale = 半高 → sizeY = 2×scale）。 */
  readonly sizeX: number;
  readonly sizeY: number;
  /** 体积盒 Z 向全长 = depth × scale。 */
  readonly boxDepth: number;
}

/** 与 decalRouting.ts 完全同口径（u32 常量）。 */
export function decalRoutePlan(materialInstance: number | null | undefined): DecalRoutePlan {
  switch ((materialInstance ?? 0) >>> 0) {
    case 0x73684efc:
      return { family: "sdf", surface: "plane" };
    case 0xe5390a98:
      return { family: "sign", surface: "projected" };
    case 0x4491de3a:
      return { family: "hole", surface: "projected" };
    default:
      return { family: "clip", surface: "projected" };
  }
}

function normalizeOrZero(v: Vec3): Vec3 {
  const len = Math.hypot(v[0], v[1], v[2]);
  // 与 three Vector3.normalize 一致：零向量归一化结果为零（不抛错）
  if (!Number.isFinite(len) || len === 0) return [0, 0, 0];
  return [v[0] / len, v[1] / len, v[2] / len];
}

/**
 * 贴花 frame；非法矩阵/scale 返回 null（与旧 decalFrame 的返回语义一致）。
 * 轴取 (m0,m1,m2)/(m3,m4,m5)/(m6,m7,m8) —— three 矩阵的列。
 */
export function decalFramePlan(
  transform: readonly number[] | null | undefined,
  scale: number | null | undefined,
  depth: number | null | undefined,
  aspectRatio: number,
): DecalFramePlan | null {
  if (!transform || transform.length !== 12) return null;
  if (scale === null || scale === undefined || !Number.isFinite(scale) || scale <= 0) {
    return null;
  }
  const m = transform;
  const aspect = aspectRatio > 0 && Number.isFinite(aspectRatio) ? aspectRatio : 1;
  const sizeY = Math.max(scale * 2, 0.05);
  return {
    origin: [m[9], m[10], m[11]],
    axisX: normalizeOrZero([m[0], m[1], m[2]]),
    axisY: normalizeOrZero([m[3], m[4], m[5]]),
    axisZ: normalizeOrZero([m[6], m[7], m[8]]),
    sizeX: Math.max(sizeY * aspect, 0.05),
    sizeY,
    boxDepth: Math.max((depth ?? 0) * scale, 0),
  };
}
