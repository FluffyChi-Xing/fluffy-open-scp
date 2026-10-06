import type * as ThreeNamespace from "three";
import type { LotUnitDto } from "@/api/tauri";
import { unitMatrix } from "../unitGizmos";
import { getTreeBillboard, getTreeModelObject } from "../propModels";
import type { UnitRenderContext, UnitRenderer } from "./types";

/**
 * 树渲染器（模型树路线，流程文档 §1.6）：descriptor 配置表的 impostor 源
 * 3D 模型直出；载荷缺席回落真图集公告板（引擎同档 128×128 精度）。
 * 仅精细模式且 resourceID 命中树资源集时适用。
 */
export const renderTreeUnit: UnitRenderer = async (
  ctx: UnitRenderContext,
  unit: LotUnitDto,
): Promise<ThreeNamespace.Object3D | null> => {
  if (
    ctx.renderMode !== "refined" ||
    unit.kind !== "prop" ||
    typeof unit.resourceId !== "number" ||
    !ctx.treeIds.has(unit.resourceId)
  ) {
    return null;
  }
  const position = new ctx.THREE.Vector3();
  const quaternion = new ctx.THREE.Quaternion();
  const scale = new ctx.THREE.Vector3();
  if (unit.transform) {
    unitMatrix(ctx.THREE, unit.transform).decompose(position, quaternion, scale);
  }
  const seed = unit.resourceId * 2654435761 + unit.index;
  return (
    (ctx.treeModelPayloads.length
      ? await getTreeModelObject(ctx.THREE, ctx.treeModelPayloads, {
          seed,
          halfWidth: unit.scale,
          position,
        })
      : null) ??
    (await getTreeBillboard(ctx.THREE, {
      seed,
      halfWidth: unit.scale,
      position,
      atlasBase64: ctx.treeAtlasPng,
    })) ??
    null
  );
};
