import { describe, expect, it } from "vitest";
import {
  PERFORMANCE_PRESETS,
  performancePlacements,
  writePerformanceBindings,
  restorePerformanceBindings,
} from "./performanceBindings";
import {
  schemaUnitJson,
  unitsFromSchema,
} from "@/pages/packages/components/property-editor/peSchemaDoc";
describe("explicit performance bindings", () => {
  it("places required anchors relative to the selected origin and reuses existing names", () => {
    const preset = PERFORMANCE_PRESETS.find(
      (p) => p.category === "college" && p.anchors.length > 1,
    )!;
    const first = performancePlacements(preset, [10, 20, 30], 0, 123, []);
    expect(first).toHaveLength(preset.anchors.length);
    expect(first[0].unit.transform?.matrix.slice(9)).toEqual([10, 20, 30]);
    const second = performancePlacements(
      preset,
      [100, 200, 300],
      first.length,
      123,
      first.map((p) => p.unit),
    );
    expect(second.every((p) => p.existing)).toBe(true);
    expect(second[0].unit.performancePresets).toEqual([preset.id]);
  });
  it("round trips optional binding metadata and preserves old documents", () => {
    const preset = PERFORMANCE_PRESETS.find((p) => p.category === "college")!;
    const units = performancePlacements(preset, [0, 0, 0], 0, 123, []).map(
      (p) => p.unit,
    );
    const doc = {
      $schema: "openscp.lot-asset/1",
      units: units.map(schemaUnitJson),
    };
    writePerformanceBindings(doc, units);
    const restored = unitsFromSchema(doc);
    restorePerformanceBindings(doc, restored);
    expect(restored[0]).toMatchObject({ performancePresets: [preset.id] });
    const legacy = {
      $schema: "openscp.lot-asset/1",
      units: units.map(schemaUnitJson),
    };
    const old = unitsFromSchema(legacy);
    restorePerformanceBindings(legacy, old);
    expect(old[0]).not.toHaveProperty("performancePresets");
  });
});
