import type * as ThreeNamespace from "three";
import type { LotUnitDto } from "@/api/tauri";
import { executeUnitPlan } from "@/render/exec/three/units";
import { planUnit } from "@/render/plan/units";
import type { UnitRenderContext, UnitRenderer } from "./types";

/**
 * 精细模式资产渲染分发（2026-10-31 迁移收尾）。
 *
 * 判定与参数选择 → `src/render/plan/units.ts`（纯函数）；
 * three 装配 → `src/render/exec/three/units.ts`（唯一副作用层）。
 * 本文件仅保留 `renderRefinedAssetUnit` 兼容入口，不再经过 adapters 桥。
 */
export async function renderRefinedAssetUnit(
  ctx: UnitRenderContext,
  unit: LotUnitDto,
): Promise<ThreeNamespace.Object3D | null> {
  const plan = planUnit(unit, {
    renderMode: ctx.renderMode,
    treeIds: ctx.treeIds,
    propModels: ctx.propModels,
    addedModelPayloads: ctx.addedModelPayloads,
    treeModelPayloadCount: ctx.treeModelPayloads.length,
    simPartsCount: ctx.simParts.length,
  });
  if (!plan) return null;
  return executeUnitPlan(plan, unit, {
    THREE: ctx.THREE,
    propModels: ctx.propModels,
    addedModelPayloads: ctx.addedModelPayloads,
    treeModelPayloads: ctx.treeModelPayloads,
    treeAtlasPng: ctx.treeAtlasPng,
    envRefs: ctx.envRefs,
    simParts: ctx.simParts,
  });
}

export type { UnitRenderContext, UnitRenderer };
