import { expect, it, vi } from "vitest";
import type { OpenScpDataSource } from "@/api/data-source";
import type { LotModelPayload, PropUnit } from "@/api/tauri";
import { planUnit } from "@/render/plan/units";
import { draftPropResourceIds, loadDraftPropModels, loadPlacedPropModel } from "./workbenchPropModels";

const payload = vi.hoisted(() => ({ glbs: [new ArrayBuffer(4)] }) as LotModelPayload);
vi.mock("@/lib/three-gltf", () => ({ parseLotModelContainer: () => payload }));

const vehicle = {
  kind: "prop", resourceId: 1, index: 0, bin: 0, transform: null,
  scale: null, slot: null, fields: [],
} satisfies PropUnit;
const tree = { ...vehicle, resourceId: 2, index: 1, bin: 9 };
const modelTgi = { typeId: 0x2f4e681b, group: 0, instance: 13 };

it("hydrates resource-only draft props and tries the next model if the first LOD fails", async () => {
  const lowerLod = { ...modelTgi, instance: 14 };
  const source = {
    resolvePropModels: vi.fn<OpenScpDataSource["resolvePropModels"]>().mockResolvedValue({
      resolutions: [
        { resourceId: 1, packageId: 3, models: [modelTgi, lowerLod], source: "script", treeAtlasPng: null },
        { resourceId: 2, packageId: null, models: [], source: "tree", treeAtlasPng: "atlas" },
      ],
    }),
    readLotModelMeshes: vi.fn().mockRejectedValueOnce(new Error("LOD unavailable"))
      .mockResolvedValue(new ArrayBuffer(0)),
  };
  const ids = draftPropResourceIds([tree, vehicle, vehicle, { ...vehicle, modelTgi, resourceId: 4 }]);
  const models = await loadDraftPropModels(source, ids);
  expect(source.resolvePropModels).toHaveBeenCalledWith([1, 2]);
  expect(source.readLotModelMeshes).toHaveBeenLastCalledWith(3, lowerLod);
  expect(models.propModels.get(1)).toBe(payload);
  expect(models.propTreeIds.has(2)).toBe(true);
  expect(models.treeAtlasPng).toBe("atlas");
});

it("binds the model read from a reopened package to the saved prop's render key", async () => {
  const placed = { ...vehicle, modelTgi, modelPackageId: 3, modelPackagePath: "models.package" };
  const source = {
    openPackage: vi.fn().mockResolvedValue({ package: { packageId: 19 } }),
    readLotModelMeshes: vi.fn().mockResolvedValue(new ArrayBuffer(0)),
  };
  const model = await loadPlacedPropModel(source, placed);
  expect(source.openPackage).toHaveBeenCalledWith(placed.modelPackagePath);
  expect(source.readLotModelMeshes).toHaveBeenCalledWith(19, modelTgi);
  expect(model).not.toBeNull();
  expect(planUnit(placed, {
    renderMode: "refined", propModels: new Map(), treeIds: new Set(),
    addedModelPayloads: new Map([[model!.key, model!.payload]]),
    treeModelPayloadCount: 0, simPartsCount: 0,
  })).toMatchObject({ kind: "prop" });
});
