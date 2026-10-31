import type * as ThreeNamespace from "three";
import type { LotUnitDto } from "@/api/tauri";
import { renderUnitPlan } from "@/render/adapters/legacyViewport";
import type { UnitRenderContext, UnitRenderer } from "./types";

/**
 * 精细模式资产渲染分发（2026-10-31 迁移）。
 *
 * 旧实现（tree/prop/spawner 三个 renderer 各自「判定 + 装配」）已迁到
 * `src/render` 的分层管线：
 *   - 判定与参数选择 → `src/render/plan/units.ts`（纯函数、可单测）
 *   - three 装配      → `src/render/exec/three/units.ts`（唯一副作用层）
 *   - 本文件只剩兼容壳：保持 `renderRefinedAssetUnit` 契约不变，
 *     视口无需改动（迁移期最小接线）。
 *
 * 返回 null = 不适用/资产缺席/解析失败，调用方统一退标记锥
 * （buildUnitObject）。旧 propRenderer/treeRenderer/spawnerRenderer 已删除
 * （2026-10-31，逻辑全部迁入 src/render）。
 */
export async function renderRefinedAssetUnit(
  ctx: UnitRenderContext,
  unit: LotUnitDto,
): Promise<ThreeNamespace.Object3D | null> {
  return renderUnitPlan(ctx, unit);
}

export type { UnitRenderContext, UnitRenderer };
