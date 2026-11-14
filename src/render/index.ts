/**
 * 新一代渲染管线入口（2026-10-31 迁移）。
 *
 * 分层：
 *   contract/  纯类型与描述符（无 three）
 *   resolve/   纯推导（矩阵/尺寸/像素核）
 *   plan/      纯计划（DTO + 事实 → 可执行描述符）
 *   exec/      three 执行器（唯一副作用层）
 *   session/   阶段编排（单元/贴花/地面）
 */
export type {
  AssetRef,
  PropUnitPlan,
  ScaleSemantics,
  SpawnerUnitPlan,
  TransformSpec,
  TreeUnitPlan,
  UnitPlan,
  UnitPlanContext,
} from "./contract/types";

export { rowMajor12ToTrs, quaternionFromRotationRows } from "./resolve/transform";
export {
  affineFromRowMajor12,
  affineToColumnMajor16,
  invertAffine,
  type AffineRows,
} from "./resolve/matrix";
export {
  composeGroundPixels,
  groundOutputSize,
  type GroundComposeInput,
  type GroundComposeOutput,
  type Pixels,
} from "./resolve/groundCompose";

export { directAssetKey, planUnit, unitPlanId } from "./plan/units";
export {
  lotDimensionPlan,
  lotGroundPlan,
  type DimensionAnchor,
  type LotDimensionPlan,
  type LotGroundPlan,
} from "./plan/ground";
export {
  decalFramePlan,
  decalRoutePlan,
  type DecalFamily,
  type DecalFramePlan,
  type DecalRoutePlan,
  type DecalSurface,
} from "./plan/decal";
export {
  planMeshMaterial,
  planMeshMaterials,
  planModel,
  type MeshMaterialPlan,
  type MeshMaterialStrategy,
  type ModelPlan,
} from "./plan/model";

export { executeUnitPlan, type UnitExecutionResources } from "./exec/three/units";
export { executeModel, type ModelExecutionInput } from "./exec/three/model";
export { createLotDimensions, createLotGround } from "./exec/three/ground";
export { applyGroundTextures } from "./exec/three/groundTextures";
export { buildDecalBody, type DecalBodyInput, type DecalBodyState } from "./exec/three/decalBody";
export {
  buildPathLine,
  buildRealLightUnit,
  buildSpawnerPlaceholder,
  buildUnitObject,
} from "./exec/three/overlays";

export {
  kindGroup,
  stageDecals,
  stageUnits,
  type DecalStageInput,
  type UnitStageInput,
} from "./session/stages";
export { stageGround, type GroundStageInput } from "./session/groundStage";
export {
  RELIEF_CONSTANTS,
  planRelief,
  reliefHasHeight,
  type ReliefPlan,
  type ReliefPlanInput,
} from "./plan/relief";
export { RELIEF_GLSL } from "./exec/three/reliefShader";
export {
  classifyAssetRef,
  classifyProp,
  type PropSemanticFacts,
} from "./plan/propSemantic";
