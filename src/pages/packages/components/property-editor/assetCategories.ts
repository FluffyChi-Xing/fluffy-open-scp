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
/** i18n 键版类别标签（EN 模式翻译不完全修正，2026-10-27）。新增导出，
 * 既有 CATEGORY_LABELS 保留兼容；消费方用 $t(key) 渲染。 */
export const CATEGORY_LABEL_KEYS: Record<AssetCategory, string> = {
  trees: "package.category.trees",
  vehicles: "package.category.vehicles",
  waste: "package.category.waste",
  furniture: "package.category.furniture",
  buildings: "package.category.buildings",
  other: "package.category.other",
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

/** A prop bin is an engine slot; resourceId resolves its actual meaning. */
export function propSemanticCategory(
  resourceId: number | null | undefined,
  name = "",
  trees?: Set<number>,
): Exclude<AssetCategory, "buildings"> {
  const category = assetCategory(resourceId ?? null, name, trees);
  return category === "buildings" ? "other" : category;
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

/** unitCategory 的 i18n 键版：返回 locale 键（decal 附带 {n} 参数），
 * 消费方 $t(key, params) 渲染；分组归类逻辑与 unitCategory 逐字一致。 */
export function unitCategoryKey(
  unit: LotUnitDto,
  trees?: Set<number>,
  categories?: Map<number, AssetCategory>,
): { key: string; params?: Record<string, number> } {
  if (unit.kind === "prop")
    return {
      key: CATEGORY_LABEL_KEYS[
        (unit.resourceId != null && categories?.get(unit.resourceId)) ||
          assetCategory(unit.resourceId, unit.displayName, trees)
      ],
    };
  if (unit.kind === "light")
    return {
      key:
        {
          Point: "package.unitCategory.lightPoint",
          Spot: "package.unitCategory.lightSpot",
          Line: "package.unitCategory.lightLine",
        }[unit.lightType ?? "Point"] ?? "package.unitCategory.lightOther",
    };
  if (unit.kind === "decal")
    return {
      key: "package.unitCategory.decalCategory",
      params: { n: unit.category + 1 },
    };
  return unit.kind === "spawner"
    ? { key: "package.unitCategory.spawner" }
    : unit.kind === "pathPoint"
      ? { key: "package.unitCategory.pathPoint" }
      : { key: "package.unitCategory.effect" };
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
