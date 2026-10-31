/**
 * 单元计划执行器（唯一接触 three 的一层）。
 *
 * 语义与旧 `renderers/{tree,prop,spawner}Renderer` 完全一致；区别只在于：
 * 判定与参数选择在 `plan/units.ts`（纯函数、可单测），这里只负责取载荷、
 * 调材质/几何工厂、写 Object3D 与增量缩放标记。
 */
import type * as ThreeNamespace from "three";
import type { LotModelPayload, LotUnitDto } from "@/api/tauri";
import type { SunEnvRefs } from "@/pages/packages/components/property-editor/refinedRender";
import type { SimPart } from "@/pages/packages/components/property-editor/simAssets";
import { getSimPopulation } from "@/pages/packages/components/property-editor/simAssets";
import {
  getPropModelObject,
  getTreeBillboard,
  getTreeModelObject,
} from "@/pages/packages/components/property-editor/propModels";
import type {
  AssetRef,
  TransformSpec,
  TreeUnitPlan,
  UnitPlan,
} from "../../contract/types";

export interface UnitExecutionResources {
  readonly THREE: typeof ThreeNamespace;
  readonly propModels: ReadonlyMap<number, LotModelPayload>;
  readonly addedModelPayloads?: ReadonlyMap<string, LotModelPayload>;
  readonly treeModelPayloads: readonly LotModelPayload[];
  readonly treeAtlasPng: string | null;
  readonly envRefs: SunEnvRefs | null;
  readonly simParts: readonly SimPart[];
}

export function resolveAssetPayload(
  asset: AssetRef,
  resources: UnitExecutionResources,
): LotModelPayload | null {
  if (asset.kind === "resource") {
    return resources.propModels.get(asset.resourceId) ?? null;
  }
  return resources.addedModelPayloads?.get(asset.key) ?? null;
}

function applyTransform(
  THREE: typeof ThreeNamespace,
  transform: TransformSpec | null,
  position: ThreeNamespace.Vector3,
  quaternion: ThreeNamespace.Quaternion,
  scale: ThreeNamespace.Vector3,
): void {
  if (!transform) return;
  position.set(transform.position[0], transform.position[1], transform.position[2]);
  quaternion.set(
    transform.quaternion[0],
    transform.quaternion[1],
    transform.quaternion[2],
    transform.quaternion[3],
  );
  scale.set(transform.scale[0], transform.scale[1], transform.scale[2]);
}

async function executeTree(
  plan: TreeUnitPlan,
  resources: UnitExecutionResources,
): Promise<ThreeNamespace.Object3D | null> {
  const { THREE } = resources;
  const position = new THREE.Vector3();
  const quaternion = new THREE.Quaternion();
  const scale = new THREE.Vector3(1, 1, 1);
  applyTransform(THREE, plan.transform, position, quaternion, scale);

  if (plan.source === "direct") {
    const payload = resolveAssetPayload(plan.asset, resources);
    if (!payload) return null;
    const scalar = plan.dtoScale ?? 1;
    scale.multiplyScalar(scalar);
    const tree = await getTreeModelObject(THREE, [payload], {
      seed: plan.unitIndex,
      position,
      scale,
    });
    if (!tree) return null;
    tree.quaternion.copy(quaternion);
    tree.userData.incrementalScale = scalar;
    return tree;
  }

  // resolved：treeIds 命中；载荷来自 treeModelPayloads（不是 propModels）
  if (plan.dtoScale !== null) scale.multiplyScalar(plan.dtoScale);
  const model = resources.treeModelPayloads.length
    ? await getTreeModelObject(THREE, [...resources.treeModelPayloads], {
        seed: plan.seed,
        scale,
        position,
      })
    : null;
  const object =
    model ??
    (await getTreeBillboard(THREE, {
      seed: plan.seed,
      halfWidth: plan.dtoScale,
      position,
      atlasBase64: resources.treeAtlasPng,
    }));
  if (!object) return null;
  // 模型树 scale 含 DTO 烘焙；公告板尺寸自持（只跟位置/朝向）
  object.userData.incrementalScale = model ? (plan.dtoScale ?? 1) : "keepScale";
  return object;
}

async function executeProp(
  plan: Extract<UnitPlan, { kind: "prop" }>,
  resources: UnitExecutionResources,
): Promise<ThreeNamespace.Object3D | null> {
  const { THREE } = resources;
  const payload = resolveAssetPayload(plan.asset, resources);
  if (!payload) return null;
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
  // 缩放倍率烘焙标记（增量更新契约，显式化为 ScaleSemantics）
  propModel.userData.incrementalScale = plan.dtoScale ?? 1;
  if (plan.dtoScale !== null) propModel.scale.multiplyScalar(plan.dtoScale);
  return propModel;
}

async function executeSpawner(
  plan: Extract<UnitPlan, { kind: "spawner" }>,
  unit: LotUnitDto,
  resources: UnitExecutionResources,
): Promise<ThreeNamespace.Object3D | null> {
  const { THREE } = resources;
  if (!resources.simParts.length) return null;
  if (unit.kind !== "spawner") return null;
  const position = new THREE.Vector3();
  const quaternion = new THREE.Quaternion();
  applyTransform(THREE, plan.transform, position, quaternion, new THREE.Vector3());
  const population = await getSimPopulation(THREE, [...resources.simParts], unit);
  if (!population) return null;
  population.position.copy(position);
  population.quaternion.copy(quaternion);
  // 人形尺寸自持（合成骨架自带比例，矩阵缩放不参与渲染）
  population.userData.incrementalScale = "keepScale";
  population.userData.unitLocationPreview = true;
  return population;
}

/** 计划 → Object3D；返回 null 表示资产缺席/解析失败，调用方退标记锥。 */
export async function executeUnitPlan(
  plan: UnitPlan,
  unit: LotUnitDto,
  resources: UnitExecutionResources,
): Promise<ThreeNamespace.Object3D | null> {
  if (plan.kind === "tree") return executeTree(plan, resources);
  if (plan.kind === "prop") return executeProp(plan, resources);
  return executeSpawner(plan, unit, resources);
}
