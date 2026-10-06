import type * as ThreeNamespace from "three";
import type { LotUnitDto } from "@/api/tauri";
import { buildSpawnerPlaceholder, unitMatrix } from "../unitGizmos";
import { getSimFigure } from "../simAssets";
import type { UnitRenderContext, UnitRenderer } from "./types";

/**
 * spawner 渲染器（agent 刷新锚点，props 文档 §3.1）：真小人 =
 * 身体(3 型) + 头(20 型) 按 unit id 种子确定性合成（全局资产 SIMF 通道）；
 * 资产缺席退占位人形（自带变换）。默认模式不适用（标记锥）。
 */
export const renderSpawnerUnit: UnitRenderer = async (
  ctx: UnitRenderContext,
  unit: LotUnitDto,
): Promise<ThreeNamespace.Object3D | null> => {
  if (ctx.renderMode !== "refined" || unit.kind !== "spawner") return null;
  if (!ctx.simParts.length) return null;
  const position = new ctx.THREE.Vector3();
  const quaternion = new ctx.THREE.Quaternion();
  const scale = new ctx.THREE.Vector3();
  if (unit.transform) {
    unitMatrix(ctx.THREE, unit.transform).decompose(position, quaternion, scale);
  }
  const figure = await getSimFigure(ctx.THREE, ctx.simParts, unit);
  if (!figure) return null;
  figure.position.copy(position);
  figure.quaternion.copy(quaternion);
  figure.position.z = Math.max(0, figure.position.z);
  // 人形尺寸自持（合成骨架自带比例，矩阵缩放不参与渲染）→ 增量路径
  // 只跟位置/朝向
  figure.userData.incrementalScale = "keepScale";
  return figure;
};
