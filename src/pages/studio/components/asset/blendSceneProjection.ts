import type { LotUnitDto } from "@/api/tauri";

/** Extra light controls omitted by the frozen legacy schema adapter. */
const LIGHT_CONTROLS = [
  "specular",
  "falloffStart",
  "volumetricStrength",
] as const;
export function writeSceneLightControls(
  document: Record<string, unknown>,
  units: readonly LotUnitDto[],
) {
  const rows = document.units as Record<string, unknown>[];
  for (const unit of units) {
    if (unit.kind !== "light") continue;
    const row = rows.find(
      (row) => row.kind === "light" && row.index === unit.index,
    );
    if (!row) continue;
    for (const key of LIGHT_CONTROLS)
      if (unit[key] !== undefined) row[key] = unit[key];
  }
}
export function restoreSceneLightControls(
  document: Record<string, unknown>,
  units: LotUnitDto[],
) {
  const rows = document.units as Record<string, unknown>[];
  for (const unit of units) {
    if (unit.kind !== "light") continue;
    const row = rows.find(
      (row) => row.kind === "light" && row.index === unit.index,
    );
    if (!row) continue;
    for (const key of LIGHT_CONTROLS) {
      const value = row[key];
      if (
        value === null ||
        (typeof value === "number" && Number.isFinite(value))
      )
        unit[key] = value;
    }
  }
}
