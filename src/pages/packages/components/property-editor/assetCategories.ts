import type { LotUnitDto, PropUnit } from "@/api/tauri";

export type AssetCategory =
  "trees" | "vehicles" | "waste" | "furniture" | "buildings" | "other";
export const CATEGORY_LABELS: Record<AssetCategory, string> = {
  trees: "树木 / 植被",
  vehicles: "车辆",
  waste: "垃圾桶 / 垃圾箱",
  furniture: "街道家具",
  buildings: "建筑",
  other: "其他模型",
};
// Resource identities verified in props-effects-spawners-paths-engine-flow.md.
const KNOWN: Record<number, AssetCategory> = {
  0x0ab4dfe1: "vehicles",
  0x0ab4dfe2: "vehicles",
  0x0ab4dfe3: "vehicles",
  0x92bee95f: "vehicles",
  0x54ca89f0: "vehicles",
  0x85271637: "waste",
  0x903a704c: "waste",
  0x9375c65e: "furniture",
  0xad64cb3d: "furniture",
  0x4df43690: "trees",
  0xc2fbd178: "trees",
  0x1113b131: "trees",
  0x89d658df: "trees",
};
export function assetCategory(
  id: number | null,
  name = "",
  trees?: Set<number>,
): AssetCategory {
  if (id != null && trees?.has(id)) return "trees";
  if (id != null && KNOWN[id >>> 0]) return KNOWN[id >>> 0];
  if (/tree|plant|bush|palm|foliage|树|植被/i.test(name)) return "trees";
  if (/truck|vehicle|car(?:_|\b)|bus|train|车辆|汽车/i.test(name))
    return "vehicles";
  if (/trash|dumpster|garbage|waste|垃圾/i.test(name)) return "waste";
  if (/bench|fence|chair|table|furniture|长椅|围栏/i.test(name))
    return "furniture";
  if (/building|house|station|tower|建筑/i.test(name)) return "buildings";
  return "other";
}
export function unitCategory(
  unit: LotUnitDto,
  trees?: Set<number>,
  categories?: Map<number, AssetCategory>,
): string {
  if (unit.kind === "prop")
    return CATEGORY_LABELS[
      (unit.resourceId != null && categories?.get(unit.resourceId)) ||
        assetCategory(unit.resourceId, unit.displayName, trees)
    ];
  if (unit.kind === "light")
    return (
      { Point: "点光源", Spot: "聚光灯", Line: "线光源" }[
        unit.lightType ?? "Point"
      ] ?? "其他灯光"
    );
  if (unit.kind === "decal") return `贴花类别 ${unit.category + 1}`;
  return unit.kind === "spawner"
    ? "代理生成器"
    : unit.kind === "pathPoint"
      ? "路径点"
      : "特效";
}
export function placementMatrix(
  position: [number, number, number],
  scale = 1,
): number[] {
  return [scale, 0, 0, 0, scale, 0, 0, 0, scale, ...position];
}
export function clonePropForPlacement(
  template: PropUnit,
  index: number,
  position: [number, number, number],
): PropUnit {
  const copy = JSON.parse(JSON.stringify(template)) as PropUnit;
  copy.index = index;
  copy.transform = {
    matrix: [
      ...(template.transform?.matrix ?? placementMatrix([0, 0, 0])).slice(0, 9),
      ...position,
    ],
  };
  return copy;
}

/** Runtime cache identity must include package + full TGI, not only instance. */
export function directModelKey(
  unit: Pick<PropUnit, "modelTgi" | "modelPackageId">,
): string {
  const t = unit.modelTgi;
  return t ? `${unit.modelPackageId}:${t.typeId}:${t.group}:${t.instance}` : "";
}
