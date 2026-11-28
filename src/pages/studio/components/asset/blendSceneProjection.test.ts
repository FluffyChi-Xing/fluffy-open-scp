import { describe, expect, it } from "vitest";
import type { LotUnitDto } from "@/api/tauri";
import {
  schemaUnitJson,
  unitsFromSchema,
} from "@/pages/packages/components/property-editor/peSchemaDoc";
import {
  writeSceneLightControls,
  restoreSceneLightControls,
} from "./blendSceneProjection";

describe("imported scene light controls", () => {
  it("survives schema save and restore through the frozen legacy adapter", () => {
    const units = [
      {
        kind: "light",
        index: 0,
        fields: [],
        specular: 0.25,
        falloffStart: 3,
        volumetricStrength: 0.4,
        transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 10, 20, 30] },
      },
    ] as unknown as LotUnitDto[];
    const document = {
      $schema: "openscp.lot-asset/1",
      units: units.map(schemaUnitJson),
    };
    writeSceneLightControls(document, units);
    const restored = unitsFromSchema(document);
    restoreSceneLightControls(document, restored);
    expect(restored[0]).toMatchObject({
      specular: 0.25,
      falloffStart: 3,
      volumetricStrength: 0.4,
    });
  });
});
