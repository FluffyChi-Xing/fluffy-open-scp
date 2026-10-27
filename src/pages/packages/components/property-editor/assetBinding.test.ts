import { describe, expect, it } from "vitest";
import { BUILDING_BINDING_SLOTS, validateAssetBinding } from "./assetBinding";

const valid = () => ({
  source: { path: "assets/tower.glb", format: "glb" as const, coordinateSystem: "z-up" as const, unit: "meters" as const, lodCount: 2 },
  slots: Object.fromEntries(BUILDING_BINDING_SLOTS.map((slot) => [slot, { source: `${slot}.${slot === "params" ? "json" : "png"}`, kind: slot === "params" ? "json" : "png" }])) as Record<string, { source: string; kind: "json" | "png" }>,
  menu: { menu2: { typeId: 1, group: 2, instance: 3 }, menuUnit: { typeId: 1, group: 2, instance: 4 } },
});

describe("asset binding contract", () => {
  it("accepts a complete Blender/building4/Menu2 binding", () => {
    expect(validateAssetBinding(valid())).toEqual([]);
  });

  it("requires every building slot and the correct file kind", () => {
    const contract = valid();
    delete contract.slots.interior;
    contract.slots.tint.kind = "json";
    const issues = validateAssetBinding(contract);
    expect(issues).toEqual(expect.arrayContaining([
      { code: "slot-missing", path: "slots.interior" },
      { code: "slot-kind", path: "slots.tint.kind" },
    ]));
  });

  it("rejects an unnormalised Blender source", () => {
    const contract = valid();
    contract.source.coordinateSystem = "z-up";
    contract.source.unit = "meters";
    contract.source.lodCount = 0;
    contract.source.path = "";
    expect(validateAssetBinding(contract).map((issue) => issue.code)).toEqual([
      "source-missing",
      "source-lods",
    ]);
  });
});

