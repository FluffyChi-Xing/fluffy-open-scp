/**
 * 临街朝向：作者 rotationDegrees（0/90/180/270）↔ 编辑器箭头边。
 *
 * 游戏 lot 本地系 +Y = 临街侧；Building40 默认 rotationDegrees=180
 * （Blender 白模实证）。编辑器箭头默认仍落在 +Y：
 *   streetFacingFromRotation(180) === 0
 * 切换四边时箭头跟着走，构建写入对应 rotationDegrees。
 */

import type { StreetFacingDegrees } from "@/render/plan/ground";

export type { StreetFacingDegrees };

export const DEFAULT_ROTATION_DEGREES: StreetFacingDegrees = 180;

export const STREET_FACING_OPTIONS = [
  { facing: 0 as StreetFacingDegrees, edge: "posY" as const },
  { facing: 90 as StreetFacingDegrees, edge: "negX" as const },
  { facing: 180 as StreetFacingDegrees, edge: "negY" as const },
  { facing: 270 as StreetFacingDegrees, edge: "posX" as const },
];

export function normalizeRotationDegrees(raw: unknown): StreetFacingDegrees {
  const value = typeof raw === "number" ? raw : Number(raw);
  if (value === 0 || value === 90 || value === 180 || value === 270) {
    return value;
  }
  return DEFAULT_ROTATION_DEGREES;
}

/** rotationDegrees → 箭头边角（相对 +Y，逆时针）。默认 180 → 0（+Y）。 */
export function streetFacingFromRotation(
  rotation: StreetFacingDegrees,
): StreetFacingDegrees {
  return ((rotation + 180) % 360) as StreetFacingDegrees;
}

/** 箭头边角 → rotationDegrees。+Y(0) → 180。 */
export function rotationFromStreetFacing(
  facing: StreetFacingDegrees,
): StreetFacingDegrees {
  return ((facing + 180) % 360) as StreetFacingDegrees;
}
