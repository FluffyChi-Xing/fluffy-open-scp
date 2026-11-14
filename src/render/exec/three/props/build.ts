/**
 * Prop 模型构建（exec 共用底层）：只做 three 装配，不决定语义。
 * 各语义处理器（vehicle/static/waste/sign/product）只负责选择策略，
 * 保持单一职责；语义标签来自纯计划层。
 */
import type * as ThreeNamespace from "three";
import type { LotModelPayload } from "@/api/tauri";
import type { SunEnvRefs } from "@/pages/packages/components/property-editor/refinedRender";
import { getPropModelObject } from "@/pages/packages/components/property-editor/propModels";
import type { PropSemantic, PropUnitPlan } from "../../../contract/types";
import { applyTransform } from "../transform";

export interface ModelPropResources {
  readonly THREE: typeof ThreeNamespace;
  readonly envRefs: SunEnvRefs | null;
}

export async function buildModelProp(
  plan: PropUnitPlan,
  payload: LotModelPayload,
  resources: ModelPropResources,
): Promise<ThreeNamespace.Object3D | null> {
  const { THREE } = resources;
  const propModel = await getPropModelObject(
    THREE,
    payload,
    resources.envRefs ?? undefined,
    plan.unitIndex,
  );
  if (!propModel) return null;
  const position = new THREE.Vector3();
  const quaternion = new THREE.Quaternion();
  const scale = new THREE.Vector3(1, 1, 1);
  applyTransform(THREE, plan.transform, position, quaternion, scale);
  propModel.position.copy(position);
  propModel.quaternion.copy(quaternion);
  propModel.scale.copy(scale);
  propModel.userData.incrementalScale = plan.dtoScale ?? 1;
  propModel.userData.propSemantic = plan.semantic;
  if (plan.dtoScale !== null) propModel.scale.multiplyScalar(plan.dtoScale);
  return propModel;
}

/** 语义处理器统一签名（由 props/index.ts 分发）。 */
export type PropHandler = (
  plan: PropUnitPlan,
  payload: LotModelPayload,
  resources: ModelPropResources,
) => Promise<ThreeNamespace.Object3D | null>;

export function tagSemantic(
  object: ThreeNamespace.Object3D | null,
  semantic: PropSemantic,
): ThreeNamespace.Object3D | null {
  if (object) object.userData.propSemantic = semantic;
  return object;
}
