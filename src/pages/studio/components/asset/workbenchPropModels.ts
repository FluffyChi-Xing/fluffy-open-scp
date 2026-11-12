import type { OpenScpDataSource } from "@/api/data-source";
import type { LotModelPayload, LotUnitDto, PropUnit, Tgi } from "@/api/tauri";
import { parseLotModelContainer } from "@/lib/three-gltf";
import { directAssetKey } from "@/render/plan/units";

type PropSource = Pick<OpenScpDataSource, "resolvePropModels" | "readLotModelMeshes">;

export function draftPropResourceIds(units: readonly LotUnitDto[]): number[] {
  const ids = new Set<number>();
  for (const unit of units) {
    if (unit.kind === "prop" && !unit.modelTgi && typeof unit.resourceId === "number") {
      ids.add(unit.resourceId);
    }
  }
  return [...ids].sort((left, right) => left - right);
}

export function emptyDraftPropModels() {
  return {
    propModels: new Map<number, LotModelPayload>(),
    propTreeIds: new Set<number>(),
    treeModelPayloads: [] as LotModelPayload[],
    treeAtlasPng: null as string | null,
    referencedModels: [] as { packageId: number; instance: number; group: number; resourceId: number }[],
  };
}

export async function loadDraftPropModels(source: PropSource, ids: number[]) {
  const result = emptyDraftPropModels();
  const { resolutions } = await source.resolvePropModels(ids);
  async function readModel(packageId: number, tgi: Tgi) {
    try {
      const payload = parseLotModelContainer(await source.readLotModelMeshes(packageId, tgi));
      return payload.glbs.length ? payload : null;
    } catch {
      return null;
    }
  }
  const treeModel = resolutions.find((entry) =>
    entry.source === "tree_model" && entry.packageId != null && entry.models.length,
  );
  await Promise.all(resolutions.map(async (entry) => {
    const isTree = entry.source === "tree" || entry.source === "tree_model";
    if (isTree) result.propTreeIds.add(entry.resourceId);
    result.treeAtlasPng ??= entry.treeAtlasPng ?? null;
    const packageId = entry.packageId;
    if (packageId == null) return;
    for (const model of entry.models) {
      result.referencedModels.push({
        packageId, group: model.group, instance: model.instance, resourceId: entry.resourceId,
      });
    }
    if (isTree) return;
    for (const model of entry.models) {
      const payload = await readModel(packageId, model);
      if (payload) {
        result.propModels.set(entry.resourceId, payload);
        return;
      }
    }
  }));
  if (treeModel?.packageId != null) {
    const packageId = treeModel.packageId;
    result.treeModelPayloads = (await Promise.all(
      treeModel.models.map((model) => readModel(packageId, model)),
    )).filter((payload): payload is LotModelPayload => payload !== null);
  }
  return result;
}

export async function loadPlacedPropModel(
  source: Pick<OpenScpDataSource, "openPackage" | "readLotModelMeshes">,
  unit: PropUnit,
) {
  if (!unit.modelTgi) return null;
  const packageId = unit.modelPackagePath
    ? (await source.openPackage(unit.modelPackagePath)).package.packageId
    : unit.modelPackageId;
  if (packageId == null) return null;
  const payload = parseLotModelContainer(await source.readLotModelMeshes(packageId, unit.modelTgi));
  // The reopened handle is for reading only. The render plan still uses the
  // identity in the authored unit, which may contain a handle from an older session.
  return payload.glbs.length ? { key: directAssetKey(unit), payload } : null;
}
