import { describe, expect, it } from "vitest";
import type { SpawnerUnit } from "@/api/tauri";
import { getSpawnerPopulationPreview } from "./simAssets";

function spawner(index: number): SpawnerUnit {
  return {
    kind: "spawner",
    index,
    transform: null,
    id: null,
    count: null,
    countRandom: null,
    agent: null,
    fields: [],
  };
}

describe("spawner population preview", () => {
  it("is deterministic for the same UnitLocation", () => {
    const unit = spawner(4);
    expect(getSpawnerPopulationPreview(unit)).toEqual(
      getSpawnerPopulationPreview(unit),
    );
  });

  it("samples inactive anchors and multi-agent populations", () => {
    const samples = Array.from({ length: 64 }, (_, index) =>
      getSpawnerPopulationPreview(spawner(index)),
    );
    expect(samples.some((sample) => !sample.active && sample.count === 0)).toBe(true);
    expect(
      samples.some(
        (sample) => sample.active && sample.count >= 2 && sample.count <= 5,
      ),
    ).toBe(true);
  });
});
