/**
 * 路径类型目录（2026-11-07）：作者只选"人行道/车辆引道"，entry 与
 * scope/roadConnect 全部由类型配方换算（构建器 `path_catalog.rs` 是同一
 * 份配方的权威实现；本文件是编辑器的只读投影与纯函数层）。
 *
 * 证据基线：docs/design/path-semantic-authoring.md §3.2（原版 entry 聚类，
 * Reference-first——不新建 PathEntry，零索引预算风险）。
 */

export type PathTypeId = "pedestrian" | "driveway" | "custom";

export interface PathTypeSpec {
  id: PathTypeId;
  /** i18n 标签 key（studio.assetWorkbench.pathType.<id>）。 */
  labelKey: string;
  /** i18n 说明 key。 */
  hintKey: string;
  icon: string;
  /** 原版 PathEntry instance（custom 为 null）；仅诊断/清单展示。 */
  entry: string | null;
  scope: "local" | "global";
  roadConnect: boolean;
}

/** 首版类型表（与 Rust PATH_TYPES 对齐；custom 是高级模式）。 */
export const PATH_TYPES: readonly PathTypeSpec[] = [
  {
    id: "pedestrian",
    labelKey: "studio.assetWorkbench.pathType.pedestrian",
    hintKey: "studio.assetWorkbench.pathType.pedestrianHint",
    icon: "Footprints",
    entry: "418A18B1",
    scope: "local",
    roadConnect: false,
  },
  {
    id: "driveway",
    labelKey: "studio.assetWorkbench.pathType.driveway",
    hintKey: "studio.assetWorkbench.pathType.drivewayHint",
    icon: "Car",
    entry: "7C9C4FFF",
    scope: "global",
    roadConnect: true,
  },
  {
    id: "custom",
    labelKey: "studio.assetWorkbench.pathType.custom",
    hintKey: "studio.assetWorkbench.pathType.customHint",
    icon: "Settings2",
    entry: null,
    scope: "local",
    roadConnect: false,
  },
];

export const DEFAULT_PATH_TYPE: PathTypeId = "pedestrian";

const BY_ID = new Map<PathTypeId, PathTypeSpec>(
  PATH_TYPES.map((spec) => [spec.id, spec]),
);

/** 目录条目（未知 id 回退 custom——校验层会因缺 entry 报错）。 */
export function pathTypeSpec(id: string | null | undefined): PathTypeSpec {
  return BY_ID.get((id ?? "") as PathTypeId) ?? BY_ID.get("custom")!;
}

export function isPathTypeId(value: unknown): value is PathTypeId {
  return typeof value === "string" && BY_ID.has(value as PathTypeId);
}

/** 类型 → 引擎语义（编辑器展示用；构建器用 Rust 同一份配方写列）。 */
export interface PathSemantics {
  entry: string | null;
  scope: "local" | "global";
  roadConnect: boolean;
}

export function pathSemantics(
  type: PathTypeId,
  entry: string | null,
): PathSemantics {
  const spec = pathTypeSpec(type);
  if (type === "custom") {
    return { entry: entry?.trim() || null, scope: spec.scope, roadConnect: spec.roadConnect };
  }
  return { entry: spec.entry, scope: spec.scope, roadConnect: spec.roadConnect };
}

/** 展示用：entry hex（无前缀 → 大写 0x…）。 */
export function entryHexLabel(entry: string | null): string {
  if (!entry) return "—";
  return `0x${entry.replace(/^0x/i, "").toUpperCase()}`;
}
