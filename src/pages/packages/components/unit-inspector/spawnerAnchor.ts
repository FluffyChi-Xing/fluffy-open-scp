import type { LotUnitDto, SpawnerUnit, UnitKeyDto } from "@/api/tauri";

/**
 * 生成器（spawner）锚点的纯逻辑层。
 *
 * 引擎口径（docs/re/spawner-render-and-assembly-20261101.md）：
 * lot 的 0x0E1BAC61/62 装配成 `cUnitModel::mUnitLocations` —— 一个
 * `vector_map<名字 id, 局部变换>`，**只有「名字 + 变换」两个字段**，挂在
 * non-sim（表现层）属性上；模拟层从不读它。名字是 FNV-1(lower) 哈希，
 * 引擎与演出资产按名字取点（0x141EED53 / 0x9901C8F9 为硬编码语义）。
 *
 * 因此本模块只做三件事：名字 ↔ id 换算、锚点语义分类、写入前校验。
 * 任何「一个锚点生成几个人 / 什么角色」的推断都不属于这里——那是
 * transport entry（agent 类型）的职责。
 */

/** 引擎硬编码的 UI 锚点：语音气泡 / UI 特效 / 镜头缩放的落点。 */
export const UI_ANCHOR_ID = 0x141eed53;
/** 引擎硬编码的第二个位置空间锚点（演出取点走 GetWorldLocation 同款分支）。 */
export const CAMERA_ANCHOR_ID = 0x9901c8f9;

/** 名字 id 的字节序常量（与 SP::FNV1_String8 + kCharCaseLower 同式）。 */
const FNV_OFFSET = 0x811c9dc5;
const FNV_PRIME = 0x01000193;

export type SpawnerAnchorKind = "ui" | "camera" | "custom" | "unnamed";

export type SpawnerIssueCode =
  | "anchor-missing"
  | "anchor-duplicate"
  | "transform-missing"
  | "anchor-invalid";

export interface SpawnerIssue {
  code: SpawnerIssueCode;
  /** 触发问题的单元（无则代表整批问题，如"整批都没命名"）。 */
  unitId?: string;
  /** 相关名字 id（重复项用于分组展示）。 */
  anchorId?: number;
}

export interface SpawnerAnchor {
  unitId: string;
  index: number;
  anchorId: number | null;
  kind: SpawnerAnchorKind;
  /** 十六进制显示（未命名时为空串）。 */
  label: string;
  /** 命中的引擎硬编码名字（其余为空）。 */
  knownName: SpawnerAnchorKind | null;
  position: [number, number, number] | null;
  duplicate: boolean;
}

export interface SpawnerSummary {
  total: number;
  named: number;
  unnamed: number;
  duplicates: number;
}

/** 引擎 FNV-1（乘后异或、字母折叠小写）：与非 sim 名字 id 逐字节同式。 */
export function fnv1NameId(name: string): number {
  let hash = FNV_OFFSET;
  for (let index = 0; index < name.length; index += 1) {
    const code = name.charCodeAt(index) & 0xff;
    const lower = code >= 0x41 && code <= 0x5a ? code + 0x20 : code;
    hash = Math.imul(hash, FNV_PRIME) >>> 0;
    hash = (hash ^ lower) >>> 0;
  }
  return hash >>> 0;
}

/** 名字 id → 锚点语义分类。只有两个 id 有引擎级语义，其余一律自定义。 */
export function anchorKindOf(anchorId: number | null): SpawnerAnchorKind {
  if (anchorId === null) return "unnamed";
  if (anchorId === UI_ANCHOR_ID) return "ui";
  if (anchorId === CAMERA_ANCHOR_ID) return "camera";
  return "custom";
}

export function formatAnchorId(anchorId: number | null): string {
  if (anchorId === null) return "";
  return `0x${anchorId.toString(16).toUpperCase().padStart(8, "0")}`;
}

/** 名字 key 写入口径：引擎里这两个列是「残缺 TGI」，type/group 恒 0。 */
export function anchorKey(instance: number): UnitKeyDto {
  return { typeId: 0, group: 0, instance: instance >>> 0 };
}

export function isAnchorInstanceValid(instance: number | null): boolean {
  return instance !== null && Number.isInteger(instance) && instance > 0;
}

export function readAnchorId(unit: LotUnitDto | null | undefined): number | null {
  if (!unit || unit.kind !== "spawner") return null;
  const instance = unit.id?.instance ?? null;
  return isAnchorInstanceValid(instance) ? (instance as number) >>> 0 : null;
}

export function readAnchorPosition(
  unit: LotUnitDto | null | undefined,
): [number, number, number] | null {
  if (!unit || unit.kind !== "spawner") return null;
  const matrix = unit.transform?.matrix;
  if (!matrix || matrix.length !== 12) return null;
  return [matrix[9], matrix[10], matrix[11]];
}

function unitKeyOf(unit: SpawnerUnit): string {
  return `spawner:${unit.index}`;
}

/** 锚点清单（面板与库共用）：按名字去重标记 + 已命名统计。 */
export function describeSpawners(
  units: readonly LotUnitDto[],
): SpawnerAnchor[] {
  const spawners = units.filter(
    (unit): unit is SpawnerUnit => unit.kind === "spawner",
  );
  const counts = new Map<number, number>();
  for (const unit of spawners) {
    const anchorId = readAnchorId(unit);
    if (anchorId === null) continue;
    counts.set(anchorId, (counts.get(anchorId) ?? 0) + 1);
  }
  return spawners.map((unit) => {
    const anchorId = readAnchorId(unit);
    const kind = anchorKindOf(anchorId);
    return {
      unitId: unitKeyOf(unit),
      index: unit.index,
      anchorId,
      kind,
      label: formatAnchorId(anchorId),
      knownName: kind === "custom" || kind === "unnamed" ? null : kind,
      position: readAnchorPosition(unit),
      duplicate: anchorId !== null && (counts.get(anchorId) ?? 0) > 1,
    };
  });
}

export function summarizeSpawners(units: readonly LotUnitDto[]): SpawnerSummary {
  const anchors = describeSpawners(units);
  const named = anchors.filter((anchor) => anchor.anchorId !== null).length;
  const duplicates = anchors.filter((anchor) => anchor.duplicate).length;
  return {
    total: anchors.length,
    named,
    unnamed: anchors.length - named,
    duplicates,
  };
}

/**
 * 写入前校验。三条硬规则全部来自引擎事实：
 * ① 没有名字 = 引擎无法按名取点，且写入时 ids 列会被短列截断（后面的锚点全丢）；
 * ② 同名 = `vector_map` 后写覆盖先写，锚点互相吞掉；
 * ③ 没有变换 = transforms 列同样被截断。
 */
export function spawnerIssues(
  units: readonly LotUnitDto[],
): SpawnerIssue[] {
  const issues: SpawnerIssue[] = [];
  const seen = new Map<number, string[]>();
  for (const unit of units) {
    if (unit.kind !== "spawner") continue;
    const unitId = unitKeyOf(unit);
    const anchorId = readAnchorId(unit);
    if (anchorId === null) {
      issues.push({ code: "anchor-missing", unitId });
    } else {
      const list = seen.get(anchorId) ?? [];
      list.push(unitId);
      seen.set(anchorId, list);
    }
    if (!unit.transform?.matrix || unit.transform.matrix.length !== 12) {
      issues.push({ code: "transform-missing", unitId });
    }
  }
  for (const [anchorId, list] of seen) {
    if (list.length < 2) continue;
    for (const unitId of list) {
      issues.push({ code: "anchor-duplicate", unitId, anchorId });
    }
  }
  return issues;
}

/** 单个单元的校验结果（面板内联提示用）。 */
export function spawnerIssuesFor(
  units: readonly LotUnitDto[],
  unitId: string,
): SpawnerIssue[] {
  return spawnerIssues(units).filter((issue) => issue.unitId === unitId);
}

/**
 * 新锚点的名字建议：以 `base` 为种子递增，命中沿用 FNV 哈希后仍然唯一。
 * 返回的是「作者可读名字」，写回时再换算成 id。
 */
export function suggestAnchorName(
  units: readonly LotUnitDto[],
  base = "anchor",
): string {
  const used = new Set<number>();
  for (const unit of units) {
    if (unit.kind !== "spawner") continue;
    const anchorId = readAnchorId(unit);
    if (anchorId !== null) used.add(anchorId);
  }
  if (!used.has(fnv1NameId(base))) return base;
  for (let suffix = 2; suffix < 1000; suffix += 1) {
    const candidate = `${base}${suffix}`;
    if (!used.has(fnv1NameId(candidate))) return candidate;
  }
  return `${base}-${Date.now().toString(36)}`;
}

/** 名字是否可用（非空 + 不与其它锚点撞 id；编辑自身时允许同名）。 */
export function canUseAnchorName(
  units: readonly LotUnitDto[],
  name: string,
  selfUnitId?: string,
): boolean {
  const trimmed = name.trim();
  if (!trimmed) return false;
  const target = fnv1NameId(trimmed);
  return !units.some(
    (unit) =>
      unit.kind === "spawner" &&
      unitKeyOf(unit) !== selfUnitId &&
      readAnchorId(unit) === target,
  );
}

/** 锚点种类 → 完整 i18n key（避免组件里散落字符串拼接）。 */
export const ANCHOR_KIND_LABEL: Record<SpawnerAnchorKind, string> = {
  ui: "spawnerPanel.anchorKind.ui",
  camera: "spawnerPanel.anchorKind.camera",
  custom: "spawnerPanel.anchorKind.custom",
  unnamed: "spawnerPanel.anchorKind.unnamed",
};
