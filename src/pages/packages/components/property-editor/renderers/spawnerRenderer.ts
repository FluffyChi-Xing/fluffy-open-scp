import type * as ThreeNamespace from "three";
import type { LotUnitDto } from "@/api/tauri";
import { unitMatrix } from "../unitGizmos";
import { getSimPopulation } from "../simAssets";
import type { UnitRenderContext, UnitRenderer } from "./types";

/**
 * spawner 渲染器（UnitLocation/Agent 预览）：一个锚点不再强行对应一个
 * 小人，而是按稳定 seed 做一次运行时人口样本。激活时显示多个由 body/head
 * 组合出的示意 Agent，未激活或资产尚未到位时返回 null，由调用方保留蓝色
 * 锚点。默认模式不适用（标记锥）。
 */
export const renderSpawnerUnit: UnitRenderer = async (
  ctx: UnitRenderContext,
  unit: LotUnitDto,
): Promise<ThreeNamespace.Object3D | null> => {
  if (ctx.renderMode !== "refined" || unit.kind !== "spawner") return null;
  if (!ctx.simParts.length) return null;
  const position = new ctx.THREE.Vector3();
  const quaternion = new ctx.THREE.Quaternion();
  if (unit.transform) {
    unitMatrix(ctx.THREE, unit.transform).decompose(
      position,
      quaternion,
      new ctx.THREE.Vector3(),
    );
  }
  const population = await getSimPopulation(ctx.THREE, ctx.simParts, unit);
  if (!population) return null;
  population.position.copy(position);
  population.quaternion.copy(quaternion);
  // 人形尺寸自持（合成骨架自带比例，矩阵缩放不参与渲染）。Unit transform
  // 的 scale 仍保持在数据层，避免把建筑作者的锚点缩放误当作人口数量。
  population.userData.incrementalScale = "keepScale";
  population.userData.unitLocationPreview = true;
  return population;
};
