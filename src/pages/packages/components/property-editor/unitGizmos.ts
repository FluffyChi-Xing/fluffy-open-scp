import type * as ThreeNamespace from "three";
import type { LotUnitDto, UnitTransformDto } from "@/api/tauri";

/**
 * Unit 稳定标识 + WPF 行主序矩阵工具（会话/属性面板/手柄共用）。
 *
 * 2026-10-31 迁移：标记锥/灯位/贴花矩形/路径线/真实光源等 three 构建器
 * 已全部迁到 `@/render/exec/three/overlays`，本文件只保留与 three 场景
 * 无关的纯工具（unitId / unitMatrix）。
 */

type Three = typeof ThreeNamespace;

/**
 * Unit 稳定标识：视口 userData、Outliner、选中态共用。
 * prop/decal 有分箱/类别维度，id 携带 bin/category。
 */
export function unitId(unit: LotUnitDto): string {
  if (unit.kind === "prop") return `prop:${unit.bin}:${unit.index}`;
  if (unit.kind === "decal") return `decal:${unit.category}:${unit.index}`;
  return `${unit.kind}:${unit.index}`;
}

/**
 * WPF Matrix3D（行主序、行向量约定、平移在末行）→ three Matrix4。
 * 列向量约定需转置：3×3 转置、平移入第 4 列。
 */
export function unitMatrix(
  THREE: Three,
  transform: UnitTransformDto,
): ThreeNamespace.Matrix4 {
  const m = transform.matrix;
  if (m.length !== 12) return new THREE.Matrix4();
  return new THREE.Matrix4().set(
    m[0], m[3], m[6], m[9],
    m[1], m[4], m[7], m[10],
    m[2], m[5], m[8], m[11],
    0, 0, 0, 1,
  );
}
