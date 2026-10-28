import type {
  DecalUnit,
  EffectUnit,
  LightUnit,
  LotUnitDto,
  PropUnit,
  SpawnerUnit,
} from "@/api/tauri";

/**
 * 单元复制规格（2026-10-28 热键）——纯逻辑层：从选中单元捕获复制规格，
 * 再由规格重建可放置的新单元。不含 Vue/会话状态；剪贴板状态与复制粘贴
 * 操作见 useUnitClipboard，热键接线在 ModAssetWorkbench。
 *
 * 范围（用户指令 2026-10-28）：
 *  - 可复制：light / effect / decal / prop / spawner；
 *  - pathPoint 不可复制——路径点只能随路径绘制创建（addPathPoint），
 *    完成路线后不能经热键复制粘贴；
 *  - 模型伪单元（"model"）不是 LotUnitDto，selectedUnit 为 null，天然排除；
 *  - 不做剪切（Ctrl+X）。
 */

/** 可经热键复制粘贴的单元类型。 */
export type CopyableUnitKind =
  "light" | "effect" | "decal" | "prop" | "spawner";

export type CopyableUnit =
  LightUnit | EffectUnit | DecalUnit | PropUnit | SpawnerUnit;

/** 复制规格：单元深拷贝（index 无意义，specToUnit 重建时重写）。 */
export type UnitClipboardSpec = CopyableUnit;

/** 单元是纯 JSON 数据（TGI/矩阵/字段行），与 clonePropForPlacement 同口径。 */
function deepCloneUnit<T extends LotUnitDto>(unit: T): T {
  return JSON.parse(JSON.stringify(unit)) as T;
}

/** 选中单元 → 复制规格；不可经热键复制的类型（pathPoint）返回 null。 */
export function captureUnitSpec(unit: LotUnitDto): UnitClipboardSpec | null {
  if (unit.kind === "pathPoint") return null;
  return deepCloneUnit(unit);
}

/** 复制规格 → 新单元：新 index，其余（变换/字段/模型引用）原样保留。 */
export function specToUnit(spec: UnitClipboardSpec, index: number): LotUnitDto {
  return { ...deepCloneUnit(spec), index };
}

/** 同类单元的下一个可用 index（max+1；与 placeUnit 分配口径一致）。 */
export function nextIndexOfKind(units: LotUnitDto[], kind: string): number {
  let max = -1;
  for (const unit of units) {
    if (unit.kind === kind && unit.index > max) max = unit.index;
  }
  return max + 1;
}
