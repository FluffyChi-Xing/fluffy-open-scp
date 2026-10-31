/**
 * 旧视口上下文的适配层：把 `UnitRenderContext` 映射到新管线。
 *
 * 这是迁移期的唯一桥：视口只认 `renderRefinedAssetUnit`，内部实现改为
 * `planUnit`（纯）+ `executeUnitPlan`（three）。后续视口迁到新壳后，
 * 本文件可删除。
 */
import type { LotUnitDto } from "@/api/tauri";
import type { UnitRenderContext } from "@/pages/packages/components/property-editor/renderers/types";
import { executeUnitPlan } from "../exec/three/units";
import { planUnit } from "../plan/units";

export async function renderUnitPlan(
  ctx: UnitRenderContext,
  unit: LotUnitDto,
): Promise<import("three").Object3D | null> {
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
