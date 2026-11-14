/**
 * 建筑模型执行器（three）：材质策略来自 `plan/model.ts`，本层只负责
 * 取贴图、建材质、挂网格、延迟贴图与阴影开关。
 *
 * 与旧 `PropertyEditorViewport.modelStage`（白模 → tint → deferred → shadows）
 * 逐段对齐；迁移期由视口传入同一批上下文（env / materialGroups / unitObjects）。
 */
import type * as ThreeNamespace from "three";
import type { LotModelPayload } from "@/api/tauri";
import { renderTelemetry } from "@/lib/renderTelemetry";
import type { SunEnvRefs } from "@/pages/packages/components/property-editor/refinedRender";
import {
  applyDeferredMaterialMaps,
  getDeferredMaps,
  getTintTextures,
  makeTintMaterial,
} from "@/pages/packages/components/property-editor/refinedRender";
import {
  planMeshMaterial,
  planModel,
  type ImportedMaterialFacts,
} from "../../plan/model";
import { rowMajor12ToTrs } from "../../resolve/transform";

export interface ModelExecutionInput {
  readonly THREE: typeof ThreeNamespace;
  readonly env: SunEnvRefs;
  readonly payload: LotModelPayload | null;
  readonly modelObjects: ThreeNamespace.Object3D[];
  readonly renderMode: "default" | "refined";
  readonly modelSource: "native" | "glb";
  readonly reliefEnabled: boolean;
  readonly modelSelectable: boolean;
  readonly modelTransform: readonly number[] | null;
  readonly modelOrigin: readonly [number, number, number] | null;
  readonly maxAnisotropy: number;
  readonly isStale: () => boolean;
  readonly specUniforms: { value: number }[];
  readonly buildingMeshes: ThreeNamespace.Mesh[];
  readonly unitObjects: Map<string, ThreeNamespace.Object3D>;
  readonly modelGroup: ThreeNamespace.Object3D;
  readonly setShadowsEnabled: (enabled: boolean) => void;
  readonly applySun: () => void;
  readonly invalidate: () => void;
}

function importedMaterialFacts(
  material: ThreeNamespace.Material | undefined,
): ImportedMaterialFacts {
  const standard = material as ThreeNamespace.MeshStandardMaterial | undefined;
  return {
    name: standard?.name ?? "",
    hasMap: Boolean(standard?.map),
    hasNormalMap: Boolean(standard?.normalMap),
    hasEmissiveMap: Boolean(standard?.emissiveMap),
  };
}

export async function executeModel(input: ModelExecutionInput): Promise<void> {
  const { THREE, payload, modelObjects, env, isStale } = input;
  const plan = planModel({
    renderMode: input.renderMode,
    modelSource: input.modelSource,
    reliefEnabled: input.reliefEnabled,
    modelSelectable: input.modelSelectable,
    modelObjectCount: modelObjects.length,
    payloadMaterialCount: payload?.materials?.length ?? 0,
    hasPayload: Boolean(payload),
    modelOrigin: input.modelOrigin,
    modelTransform: input.modelTransform,
  });

  const whiteMaterial = new THREE.MeshStandardMaterial({
    color: 0xb8c2cc,
    roughness: 0.55,
    metalness: 0.12,
    side: THREE.DoubleSide,
  });
  // 逐 mesh 材质下标（与 glbs 同序）；材质分组的下标 → 该组 mesh 的材质列表
  const materialGroups: ThreeNamespace.MeshStandardMaterial[][] = (
    payload?.materials ?? []
  ).map(() => []);

  const tintSpan = renderTelemetry.begin("texture_compose", {
    phase: "tint",
    refined: input.renderMode === "refined",
  });
  const tintResolved =
    plan.tintEnabled && payload
      ? await getTintTextures(THREE, payload, input.maxAnisotropy)
      : [];
  tintSpan.end({
    materials: payload?.materials?.length ?? 0,
    tinted: tintResolved.filter((entry) => entry?.tintTex).length,
  });
  if (isStale()) return;

  const tintReady = tintResolved.map((entry) => ({
    hasTint: Boolean(entry?.tintTex && entry.paletteTex),
  }));
  const tintPaletteRows = tintResolved.map(
    (entry) => entry?.paletteRowsByMaterial ?? new Map<string, number>(),
  );
  const payloadMaterialNames = (payload?.materials ?? []).map(
    (material) => material.materialName ?? "",
  );

  // 草稿伪单元包装组（modelSelectable）：整栋建筑作为可选中/可变换的 "model" 单元
  let modelUnit: ThreeNamespace.Group | null = null;
  if (plan.wrapModelUnit) {
    modelUnit = new THREE.Group();
    modelUnit.userData.unitId = "model";
    const trs = plan.transform ? rowMajor12ToTrs(plan.transform) : null;
    if (trs) {
      modelUnit.position.set(trs.position[0], trs.position[1], trs.position[2]);
      modelUnit.quaternion.set(
        trs.quaternion[0],
        trs.quaternion[1],
        trs.quaternion[2],
        trs.quaternion[3],
      );
      modelUnit.scale.set(trs.scale[0], trs.scale[1], trs.scale[2]);
    }
    modelUnit.userData.debugTransform = plan.transform
      ? [...plan.transform].map((value) => +value.toFixed(2))
      : null;
    input.unitObjects.set("model", modelUnit);
  }

  for (const [index, object] of modelObjects.entries()) {
    if (plan.originOffset) {
      object.position.add(new THREE.Vector3(...plan.originOffset));
    }
    const materialIndex = payload?.meshMaterialIndices[index] ?? 0;
    const uvKind = payload?.meshUvKinds[index] ?? 0;
    object.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (!mesh.isMesh) return;
      // 贴花投影的候选面（精细模式才用；建筑网格在 lot 局部空间为单位变换）
      input.buildingMeshes.push(mesh);
      // 记录 mesh 所属材质列：破洞内景需取同材质的 slot5 房间图集
      mesh.userData.buildingMaterialIndex = materialIndex;
      try {
        const importedMaterials = Array.isArray(mesh.material)
          ? mesh.material
          : [mesh.material];
        if (plan.whiteMode) {
          mesh.material = whiteMaterial;
          return;
        }
        const meshFacts = {
          materialIndex,
          uvKind,
          hasVertexColor: Boolean(mesh.geometry.attributes.color),
          importedMaterials: importedMaterials.map(importedMaterialFacts),
        };
        const strategies = importedMaterials.map((imported) =>
          planMeshMaterial({
            plan,
            materialIndex,
            materialName: imported?.name ?? "",
            mesh: meshFacts,
            tintReady,
            payloadMaterialNames,
            tintPaletteRows,
          }),
        );
        // GLB 自带贴图：整 mesh 保留 glTF 材质（旧实现同语义）
        if (strategies.some((strategy) => strategy.kind === "keepImported")) return;

        const resolvedMaterials = importedMaterials.map((imported, materialIdx) => {
          const strategy = strategies[materialIdx];
          switch (strategy.kind) {
            case "keepImported":
              return imported;
            case "vertexColor":
              return new THREE.MeshStandardMaterial({
                vertexColors: true,
                roughness: 0.82,
                metalness: 0,
                side: THREE.DoubleSide,
              });
            case "tint": {
              const tint = tintResolved[strategy.tintIndex];
              if (!tint) return whiteMaterial;
              const [tinted, specUniform] = makeTintMaterial(
                THREE,
                tint,
                env,
                plan.refined && input.reliefEnabled,
                strategy.blenderUvProfile,
                strategy.paletteRow,
                strategy.blenderAtlasUv,
                strategy.materialName,
              );
              input.specUniforms.push(specUniform);
              return tinted;
            }
            case "refined": {
              const refined = new THREE.MeshStandardMaterial({
                vertexColors: strategy.vertexColors,
                roughness: 0.82,
                metalness: 0,
                side: THREE.DoubleSide,
              });
              if (uvKind === 1) materialGroups[materialIndex]?.push(refined);
              return refined;
            }
            case "white":
            default:
              return whiteMaterial;
          }
        });
        mesh.material = Array.isArray(mesh.material)
          ? resolvedMaterials
          : resolvedMaterials[0];
      } catch (error) {
        // 逐 mesh 兜底（2026-10-08 EP1 乱码取证）：装配异常若逃出 traverse
        // 会中断整个模型线——后续 mesh 留在 GLTFLoader 默认材质。
        console.error(
          `[building-materials] glb#${index} mesh 材质装配异常:`,
          error,
        );
        mesh.material = whiteMaterial;
      }
    });
    if (import.meta.env.DEV) {
      const summaries: string[] = [];
      object.traverse((child) => {
        const mesh = child as ThreeNamespace.Mesh;
        if (!mesh.isMesh) return;
        const material = mesh.material as ThreeNamespace.MeshStandardMaterial;
        const attributes = Object.keys(mesh.geometry.attributes).join(",");
        summaries.push(
          `m${index} uvKind=${uvKind} mat=${materialIndex} ` +
            `attrs=[${attributes}] type=${material?.type} ` +
            `map=${material?.map ? "Y" : "N"} vc=${material?.vertexColors ? "Y" : "N"}`,
        );
      });
      if (summaries.length) {
        console.info(`[building-materials] glb#${index}\n${summaries.join("\n")}`);
      }
    }
    if (modelUnit) modelUnit.add(object);
    else input.modelGroup.add(object);
  }
  if (modelUnit) input.modelGroup.add(modelUnit);

  if (plan.deferredEnabled && payload) {
    const deferredSpan = renderTelemetry.begin("texture_compose", {
      phase: "deferred",
    });
    const deferredMaps = await getDeferredMaps(THREE, payload, input.maxAnisotropy);
    if (!isStale()) {
      applyDeferredMaterialMaps(deferredMaps, materialGroups);
      input.invalidate();
    }
    deferredSpan.end({ groups: materialGroups.length });
  }

  input.setShadowsEnabled(plan.shadows);
  if (plan.shadows) input.applySun();
}
