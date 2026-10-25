import type { Tgi } from "@/api/tauri";

/** The six building4 texture inputs required by the first playable asset loop. */
export const BUILDING_BINDING_SLOTS = [
  "params",
  "tint",
  "normalAo",
  "shaderMap",
  "palette",
  "interior",
] as const;

export type BuildingBindingSlot = (typeof BUILDING_BINDING_SLOTS)[number];

export interface AssetSourceBinding {
  path: string;
  format: "glb" | "lotm";
  coordinateSystem: "z-up";
  unit: "meters";
  lodCount: number;
}

export interface BuildingSlotBinding {
  source: string;
  kind: "json" | "png";
}

export interface MenuBinding {
  menu2: Tgi;
  menuUnit: Tgi;
  parentMenu?: Tgi | null;
  icon?: Tgi | null;
  marquee?: Tgi | null;
}

export interface AssetBindingContract {
  source: AssetSourceBinding;
  slots: Partial<Record<BuildingBindingSlot, BuildingSlotBinding>>;
  menu?: MenuBinding | null;
}

export interface AssetBindingIssue {
  code:
  | "source-missing"
  | "source-format"
  | "source-coordinate-system"
  | "source-units"
  | "source-lods"
  | "slot-missing"
  | "slot-kind"
  | "menu-missing"
  | "menu-tgi";
  path: string;
}

const isTgi = (value: unknown): value is Tgi => {
  if (!value || typeof value !== "object") return false;
  const row = value as Record<string, unknown>;
  return ["typeId", "group", "instance"].every(
    (key) => typeof row[key] === "number" && Number.isInteger(row[key]),
  );
};

/**
 * Validate the smallest useful Blender → PE → Menu2 contract.
 * The function is deliberately side-effect free so the importer, PE and
 * package builder can share the same gate before writing a package.
 */
export function validateAssetBinding(
  contract: Partial<AssetBindingContract> | null | undefined,
): AssetBindingIssue[] {
  const issues: AssetBindingIssue[] = [];
  const source = contract?.source;
  if (!source || !source.path.trim()) issues.push({ code: "source-missing", path: "source.path" });
  if (source && source.format !== "glb" && source.format !== "lotm")
    issues.push({ code: "source-format", path: "source.format" });
  if (source && source.coordinateSystem !== "z-up")
    issues.push({ code: "source-coordinate-system", path: "source.coordinateSystem" });
  if (source && source.unit !== "meters") issues.push({ code: "source-units", path: "source.unit" });
  if (source && (!Number.isInteger(source.lodCount) || source.lodCount < 1))
    issues.push({ code: "source-lods", path: "source.lodCount" });

  for (const slot of BUILDING_BINDING_SLOTS) {
    const binding = contract?.slots?.[slot];
    if (!binding?.source?.trim()) {
      issues.push({ code: "slot-missing", path: `slots.${slot}` });
      continue;
    }
    const expectedKind = slot === "params" ? "json" : "png";
    if (binding.kind !== expectedKind)
      issues.push({ code: "slot-kind", path: `slots.${slot}.kind` });
  }

  if (contract?.menu !== undefined && contract.menu !== null) {
    if (!isTgi(contract.menu.menu2) || !isTgi(contract.menu.menuUnit))
      issues.push({ code: "menu-tgi", path: "menu" });
  }
  return issues;
}

