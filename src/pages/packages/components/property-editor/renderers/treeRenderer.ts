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
  // 缺 transform 时恒等（Vector3 缺省 (0,0,0) 会把树缩没）
  const scale = new ctx.THREE.Vector3(1, 1, 1);
  if (unit.transform) {
    unitMatrix(ctx.THREE, unit.transform).decompose(position, quaternion, scale);
  }
  // 消防局 lot 对拍（2026-10-06）：12 棵树矩阵基缩放恒 1.0（纯旋转+平移），
  // 尺寸变化全在独立 scale 字段（0.06-0.296 三档 = 3-14m 两级观感）——
  // 与 propRenderer 真模型同规：DTO scale 必须叠加，否则整 lot 树一个尺寸。
  if (typeof unit.scale === "number") {
    scale.multiplyScalar(unit.scale);
  }
  const seed = unit.resourceId * 2654435761 + unit.index;
  const modelObject = ctx.treeModelPayloads.length
    ? await getTreeModelObject(ctx.THREE, ctx.treeModelPayloads, {
        seed,
        scale,
        position,
      })
    : null;
  const object =
    modelObject ??
    (await getTreeBillboard(ctx.THREE, {
      seed,
      halfWidth: unit.scale,
      position,
      atlasBase64: ctx.treeAtlasPng,
    })) ??
    null;
  if (object) {
    // 增量 transform 应用语义（tryIncrementalGrouping 消费）：模型树对象
    // scale = decompose×DTO scale（缺省 1）→ 增量按同式补乘；公告板尺寸
    // 是独立公式（半宽×16 钳制、center 锚点），矩阵 decompose 不可表达 →
    // 只跟位置/朝向（scale 自持，编辑经全量重建生效）。
    if (modelObject) {
      object.userData.incrementalScale =
        typeof unit.scale === "number" ? unit.scale : 1;
    } else {
      object.userData.incrementalScale = "keepScale";
    }
  }
  return object;
};
