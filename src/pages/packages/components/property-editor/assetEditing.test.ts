import { describe, expect, it } from "vitest";
import * as THREE from "three";
import {
  assetCategory,
  clonePropForPlacement,
  directModelKey,
  placementMatrix,
} from "./assetCategories";
import { schemaUnitJson, unitsFromSchema } from "./peSchemaDoc";
import { unitId, unitMatrix } from "./unitGizmos";
import type { LotUnitDto, PropUnit } from "@/api/tauri";
const tree: PropUnit = {
  kind: "prop",
  index: 3,
  bin: 2,
  resourceId: 123,
  scale: 0.12,
  slot: null,
  fields: [],
  transform: { matrix: placementMatrix([1, 2, 3]) },
};
describe("asset editing contract", () => {
  it("places a cloned tree at the requested point without losing prototype or scale", () => {
    const placed = clonePropForPlacement(tree, 9, [12, 15, 2]);
    expect(placed.resourceId).toBe(tree.resourceId);
    expect(placed.scale).toBe(0.12);
    expect(unitMatrix(THREE, placed.transform!).elements.slice(12, 15)).toEqual(
      [12, 15, 2],
    );
    expect(tree.transform!.matrix.slice(9)).toEqual([1, 2, 3]);
    expect(assetCategory(123, "", new Set([123]))).toBe("trees");
    expect(assetCategory(0x85271637)).toBe("waste");
    expect(assetCategory(null, "unrecognized")).toBe("other");
  });
  it("roundtrips unit fields including distinct spawner identity and reference", () => {
    const units: LotUnitDto[] = [
      tree,
      {
        kind: "spawner",
        index: 4,
        id: { typeId: 1, group: 3, instance: 8 },
        count: 2,
        countRandom: 0,
        agent: null,
        transform: null,
        fields: [],
      },
      {
        kind: "decal",
        index: 2,
        category: 3,
        transform: null,
        scale: 1,
        depth: 2,
        materialData: [1, 2, 3],
        fields: [],
      },
      {
        kind: "pathPoint",
        index: 1,
        point: [1, 2, 3],
        tangent: null,
        pointIndex: null,
        fields: [],
      },
    ];
    const document = {
      $schema: "openscp.lot-asset/1",
      units: units.map(schemaUnitJson),
    };
    expect(document.units.map((u) => u.id)).toEqual(units.map(unitId));
    expect(unitsFromSchema(JSON.parse(JSON.stringify(document)))).toEqual(
      units,
    );
  });
  it("distinguishes models with the same instance in different groups or packages", () => {
    const a = {
      modelPackageId: 1,
      modelTgi: { typeId: 0x2f4e681b, group: 0, instance: 123 },
    };
    expect(directModelKey(a)).not.toBe(
      directModelKey({ ...a, modelPackageId: 2 }),
    );
    expect(directModelKey(a)).not.toBe(
      directModelKey({ ...a, modelTgi: { ...a.modelTgi, group: 1 } }),
    );
  });
  it("rejects malformed placement transforms rather than silently rendering at origin", () => {
    const unit = schemaUnitJson(tree);
    unit.transform = { matrix: Array(14).fill(0) };
    expect(() =>
      unitsFromSchema({ $schema: "openscp.lot-asset/1", units: [unit] }),
    ).toThrow(/12 finite/);
  });
});
