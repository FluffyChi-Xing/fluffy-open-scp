import type * as ThreeNamespace from "three";
import type { LotUnitDto } from "@/api/tauri";
import { assetCategory, directModelKey } from "../assetCategories";
import { unitMatrix } from "../unitGizmos";
import { getPropModelObject, getTreeModelObject } from "../propModels";
import type { UnitRenderContext, UnitRenderer } from "./types";

/**
 * prop 渲染器（车辆/杂件真实模型，P2 精细替换）：脚本资源表反查的
 * LOTM 载荷装配（车漆 lerp / tint 调色链在 getPropModelObject 内）。
 *
 * ⚠ 已知开放问题（隔离在本渲染器）：消防车等车型轮胎被车漆染红——
 * 引擎口径 diffuse.a≈0 区域吃 paintColor，该车型的轮胎纹理区 alpha
 * 语义与此不符；修复时只动本文件/getPropModelObject，勿涉他族渲染器。
 * 仅精细模式且 resourceID 有载荷时适用。
 */
export const renderPropUnit: UnitRenderer = async (
  ctx: UnitRenderContext,
  unit: LotUnitDto,
): Promise<ThreeNamespace.Object3D | null> => {
  if (ctx.renderMode !== "refined" || unit.kind !== "prop") return null;
  const payload = unit.modelTgi
    ? ctx.addedModelPayloads?.get(directModelKey(unit))
    : typeof unit.resourceId === "number" ? ctx.propModels.get(unit.resourceId) : undefined;
  if (!payload) return null;
  // Raw tree kit entries use the same verified atlas decoder as resolved trees.
  if (unit.modelTgi && assetCategory(unit.modelTgi.instance, unit.displayName) === "trees") {
    const position = new ctx.THREE.Vector3();
    const rotation = new ctx.THREE.Quaternion();
    const scale = new ctx.THREE.Vector3(1, 1, 1);
    if (unit.transform) unitMatrix(ctx.THREE, unit.transform).decompose(position, rotation, scale);
    const scalar = unit.scale ?? 1;
    scale.multiplyScalar(scalar);
    const tree = await getTreeModelObject(ctx.THREE, [payload], { seed: unit.index, position, scale });
    if (tree) { tree.quaternion.copy(rotation); tree.userData.incrementalScale = scalar; }
    return tree;
  }
  const propModel = await getPropModelObject(
    ctx.THREE,
    payload,
    ctx.envRefs ?? undefined,
    unit.index,
  );
  if (!propModel) return null;
  if (unit.transform) {
    unitMatrix(ctx.THREE, unit.transform).decompose(
      propModel.position,
      propModel.quaternion,
      propModel.scale,
    );
  }
  // flags==15 hack：Scale 存于 Transform.Unknown（DTO 独立 scale 字段，
  // 半宽语义）——锥体可忽略，真模型必须叠加，否则广告牌等超出构架。
  // 叠加即烘焙：对象 scale ≠ 矩阵 decompose，增量路径须按
  // incrementalScale 补乘（见 tryIncrementalGrouping），提交须除回。
  // 恒标注数字（缺省 1）：放置单元 scale=null 起步的缩放倍率编辑才能走
  // 增量比例路径。
  propModel.userData.incrementalScale =
    typeof unit.scale === "number" ? unit.scale : 1;
  if (typeof unit.scale === "number") {
    propModel.scale.multiplyScalar(unit.scale);
  }
  return propModel;
};
