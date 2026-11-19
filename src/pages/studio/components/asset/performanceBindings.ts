import catalog from "../../../../../src-tauri/resources/sc5/performance-catalog.json";
import type { LotUnitDto, SpawnerUnit } from "@/api/tauri";
export interface PerformancePreset {
  id: string;
  category: string;
  sourceName: string;
  sourceLot: number[];
  sourceSha256: string;
  vignetteId: number[];
  definition: number[];
  sink: number[];
  anchors: { id: number; matrix: number[] }[];
}
export const PERFORMANCE_PRESETS = catalog as PerformancePreset[];
export function performancePreset(id: string) {
  return PERFORMANCE_PRESETS.find((preset) => preset.id === id);
}
export function performancePlacements(
  preset: PerformancePreset,
  position: number[],
  nextIndex: number,
  fallbackId: number,
  existing: readonly LotUnitDto[],
) {
  const slots = preset.anchors.length
    ? preset.anchors
    : [{ id: fallbackId, matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0] }];
  const origin = slots[0].matrix.slice(9);
  let index = nextIndex;
  return slots.map((slot, i): { unit: SpawnerUnit; existing: boolean } => {
    const old = existing.find(
      (u): u is SpawnerUnit =>
        u.kind === "spawner" && u.id?.instance === slot.id,
    );
    const matrix = [...slot.matrix];
    for (let axis = 0; axis < 3; axis++)
      matrix[9 + axis] = matrix[9 + axis] - origin[axis] + position[axis];
    const unit: SpawnerUnit = old
      ? { ...old }
      : {
          kind: "spawner",
          index: index++,
          id: { typeId: 0, group: 0, instance: slot.id },
          transform: { matrix },
          count: null,
          countRandom: null,
          agent: null,
          fields: [],
        };
    if (i === 0)
      unit.performancePresets = [
        ...new Set([...(unit.performancePresets ?? []), preset.id]),
      ];
    return { unit, existing: !!old };
  });
}
export function writePerformanceBindings(
  document: Record<string, unknown>,
  units: readonly LotUnitDto[],
) {
  for (const row of document.units as Record<string, unknown>[]) {
    const unit = units.find(
      (u) => u.kind === "spawner" && u.index === row.index,
    );
    if (
      row.kind === "spawner" &&
      unit?.kind === "spawner" &&
      unit.performancePresets?.length
    )
      row.performancePresets = [...unit.performancePresets];
  }
}
export function restorePerformanceBindings(
  document: Record<string, unknown>,
  units: LotUnitDto[],
) {
  for (const row of document.units as Record<string, unknown>[]) {
    const unit = units.find(
      (u) => u.kind === "spawner" && u.index === row.index,
    );
    if (
      row.kind === "spawner" &&
      unit?.kind === "spawner" &&
      Array.isArray(row.performancePresets)
    ) {
      unit.performancePresets = row.performancePresets.filter(
        (id): id is string => typeof id === "string",
      );
    }
  }
}
