/**
 * 临街吸附边（lot 本地角，相对 +Y 逆时针）。
 *
 * 游戏 lot 默认 +Y 为临街/吸附侧。本模块只表达「哪条边临街」：
 * - 编辑器橙色箭头指示该边
 * - **不**旋转模型、单位、地面或路径
 * - 与 Building40 的模型 `rotationDegrees`（几何朝向）完全正交
 */

export type StreetFacingDegrees = 0 | 90 | 180 | 270;

/** 默认临街边：+Y（游戏吸附默认侧）。 */
export const DEFAULT_STREET_FACING: StreetFacingDegrees = 0;

export const STREET_FACING_OPTIONS = [
  { facing: 0 as StreetFacingDegrees, edge: "posY" as const },
  { facing: 90 as StreetFacingDegrees, edge: "negX" as const },
  { facing: 180 as StreetFacingDegrees, edge: "negY" as const },
  { facing: 270 as StreetFacingDegrees, edge: "posX" as const },
];

export function normalizeStreetFacing(raw: unknown): StreetFacingDegrees {
  const value = typeof raw === "number" ? raw : Number(raw);
  if (value === 0 || value === 90 || value === 180 || value === 270) {
    return value;
  }
  return DEFAULT_STREET_FACING;
}

/**
 * 兼容旧 workbench.rotationDegrees（曾被误当作临街边写入）。
 * 旧口径：rotation 180 ↔ facing 0（+Y）。
 */
export function streetFacingFromLegacyRotation(
  rotation: StreetFacingDegrees,
): StreetFacingDegrees {
  return ((rotation + 180) % 360) as StreetFacingDegrees;
}
