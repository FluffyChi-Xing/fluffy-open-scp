/**
 * 建筑模型计划（纯函数）：把「渲染模式 / 资产来源 / 材质事实」变成
 * 逐 mesh 的材质策略，替代视口 `modelStage` 里的 200 行嵌套 if。
 *
 * 与旧实现的对应关系（PropertyEditorViewport.modelStage）：
 * - 默认模式恒白模（whiteMaterial 0xb8c2cc）；
 * - 外部 GLB 且无六槽材质：有贴图 → 保留 glTF 材质；有 COLOR_0 → 顶点色；
 *   否则白模；
 * - 否则逐材质：uvKind 2/3 且 tint 命中 → tint 着色链；其余 → refined
 *   MeshStandardMaterial（vertexColors 跟随几何 COLOR_0；uvKind=1 进 deferred 组）；
 * - uvKind=3（Blender 导出）：材质名反查材质列，palette 行 = 材质行 ?? (whitebox/roof ? 9.6 : 0)。
 */
import type { Vec3 } from "../contract/types";

export interface ModelPlanFlags {
  readonly renderMode: "default" | "refined";
  readonly modelSource: "native" | "glb";
  readonly reliefEnabled: boolean;
  readonly modelSelectable: boolean;
  readonly modelObjectCount: number;
  readonly payloadMaterialCount: number;
  readonly hasPayload: boolean;
  readonly modelOrigin: Vec3 | null;
  readonly modelTransform: readonly number[] | null;
}

export interface ModelPlan {
  readonly renderMode: "default" | "refined";
  readonly modelSource: "native" | "glb";
  readonly refined: boolean;
  /** getTintTextures 需要执行（精细 + 有载荷 + 原生或六槽 GLB）。 */
  readonly tintEnabled: boolean;
  /** getDeferredMaps / applyDeferredMaterialMaps 需要执行（同 tint 条件）。 */
  readonly deferredEnabled: boolean;
  /** 白模模式：所有 mesh 统一白模材质。 */
  readonly whiteMode: boolean;
  /** 是否把整栋建筑包成可选中 "model" 伪单元。 */
  readonly wrapModelUnit: boolean;
  readonly originOffset: Vec3 | null;
  readonly transform: readonly number[] | null;
  readonly shadows: boolean;
}

export function planModel(flags: ModelPlanFlags): ModelPlan {
  const refined = flags.renderMode === "refined";
  const hasSlotMaterials = flags.payloadMaterialCount > 0;
  const nativeLike = flags.modelSource !== "glb" || hasSlotMaterials;
  const tintEnabled = refined && flags.hasPayload && nativeLike;
  return {
    renderMode: flags.renderMode,
    modelSource: flags.modelSource,
    refined,
    tintEnabled,
    deferredEnabled: tintEnabled,
    whiteMode: !refined,
    wrapModelUnit: flags.modelSelectable && flags.modelObjectCount > 0,
    originOffset: flags.modelOrigin,
    transform:
      flags.modelTransform && flags.modelTransform.length === 12
        ? flags.modelTransform
        : null,
    shadows: refined,
  };
}

export interface ImportedMaterialFacts {
  readonly name: string;
  readonly hasMap: boolean;
  readonly hasNormalMap: boolean;
  readonly hasEmissiveMap: boolean;
}

export interface ModelMeshFacts {
  readonly materialIndex: number;
  readonly uvKind: number;
  readonly hasVertexColor: boolean;
  readonly importedMaterials: readonly ImportedMaterialFacts[];
}

export type MeshMaterialStrategy =
  | { readonly kind: "white" }
  | { readonly kind: "keepImported" }
  | { readonly kind: "vertexColor" }
  | {
      readonly kind: "tint";
      readonly tintIndex: number;
      readonly materialName?: string;
      readonly blenderUvProfile: boolean;
      readonly paletteRow: number;
      readonly blenderAtlasUv: boolean;
    }
  | {
      readonly kind: "refined";
      readonly vertexColors: boolean;
      readonly materialIndex: number;
    };

export interface MeshMaterialPlan {
  /** 每个 imported material 一个策略（与 mesh.material 数组同序）。 */
  readonly strategies: readonly MeshMaterialStrategy[];
  /** uvKind=1 的 refined 材质需要进 deferred 组（按材质列）。 */
  readonly deferredGroupMaterialIndexes: readonly number[];
}

export interface TintAvailability {
  readonly hasTint: boolean;
}

export interface ModelMaterialFacts {
  readonly plan: ModelPlan;
  readonly materialIndex: number;
  readonly materialName: string;
  readonly mesh: ModelMeshFacts;
  readonly tintReady: readonly TintAvailability[];
  readonly payloadMaterialNames: readonly string[];
  /** 每个 tint 材质列的 palette 行表（来自 TintTextureSet）。 */
  readonly tintPaletteRows: readonly ReadonlyMap<string, number>[];
}

/** 逐 imported material 决策；纯函数，便于枚举所有分支。 */
export function planMeshMaterial(facts: ModelMaterialFacts): MeshMaterialStrategy {
  const { plan, mesh, materialName } = facts;
  if (plan.whiteMode) return { kind: "white" };
  if (plan.modelSource === "glb" && facts.payloadMaterialNames.length === 0) {
    const hasTexture = mesh.importedMaterials.some(
      (material) => material.hasMap || material.hasNormalMap || material.hasEmissiveMap,
    );
    if (hasTexture) return { kind: "keepImported" };
    if (mesh.hasVertexColor) return { kind: "vertexColor" };
    return { kind: "white" };
  }
  const blenderMaterialIndex =
    mesh.uvKind === 3 && materialName
      ? facts.payloadMaterialNames.indexOf(materialName)
      : -1;
  const tintIndex = blenderMaterialIndex >= 0 ? blenderMaterialIndex : facts.materialIndex;
  const tint = facts.tintReady[tintIndex];
  const blenderUvProfile = mesh.uvKind === 3;
  if ((mesh.uvKind === 2 || mesh.uvKind === 3) && tint?.hasTint) {
    const isWhiteboxOrRoof = blenderUvProfile && /whitebox|roof/i.test(materialName);
    return {
      kind: "tint",
      tintIndex,
      materialName: blenderUvProfile ? materialName : undefined,
      blenderUvProfile,
      paletteRow:
        facts.tintPaletteRows[tintIndex]?.get(materialName) ??
        (isWhiteboxOrRoof ? 9.6 : 0),
      blenderAtlasUv: isWhiteboxOrRoof,
    };
  }
  return {
    kind: "refined",
    vertexColors: mesh.hasVertexColor,
    materialIndex: facts.materialIndex,
  };
}

/** 一个 mesh 的全部 imported material 计划（数组顺序与 mesh.material 一致）。 */
export function planMeshMaterials(
  facts: Omit<ModelMaterialFacts, "materialName"> & {
    readonly importedNames: readonly string[];
  },
): MeshMaterialPlan {
  const strategies = facts.importedNames.map((materialName) =>
    planMeshMaterial({ ...facts, materialName }),
  );
  const deferredGroupMaterialIndexes =
    facts.mesh.uvKind === 1
      ? strategies
          .filter((strategy) => strategy.kind === "refined")
          .map(() => facts.materialIndex)
      : [];
  return { strategies, deferredGroupMaterialIndexes };
}
