import type * as ThreeNamespace from "three";
import type { LotUnitDto } from "@/api/tauri";
import { unitMatrix } from "../unitGizmos";
import { getPropModelObject } from "../propModels";
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
  const payload =
    typeof unit.resourceId === "number"
      ? ctx.propModels.get(unit.resourceId)
      : undefined;
  if (!payload) return null;
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
  // 半宽语义）——锥体可忽略，真模型必须叠加，否则广告牌等超出构架
  if (typeof unit.scale === "number") {
    propModel.scale.multiplyScalar(unit.scale);
  }
  return propModel;
};
