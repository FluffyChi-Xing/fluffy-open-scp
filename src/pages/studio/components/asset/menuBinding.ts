import developmentMenus from "@/lib/game-ui/development-menus.json";

/** 菜单维度：城市根分类 / 城市发展产业菜单（区域维度游戏侧无容器实例，排除）。 */
export type MenuDimension = "city" | "development";

/**
 * 菜单绑定 v2（2026-11-06 菜单编辑节点组）。
 *
 * 游戏机制（s17 系列实证，见 docs/re/rci-school-path-prop-mechanism-20261105.md）：
 * 菜单归属 = Menu2 属性的 uiCategories（0x0DB9FC63）Key 数组，一个建筑身份
 * 可同时挂在多个容器下；placement 与建筑功能（Unit behaviorCategory/模板链）
 * 是两条正交边——菜单绑定只决定建筑显示在哪儿，不赋予/约束任何功能。
 *
 * 数据形态（asset.lot.json → editor.workbench.menu）：
 *   { enabled, label, description, uiPosition, entries: [{id, dimension, categoryInstance}] }
 * label/description/uiPosition 是菜单项身份（一份 Menu2 只有一份标题/描述，
 * 多容器共享），entries 每条对应一个目标菜单容器（= 画布上一个菜单编辑节点）。
 *
 * deriveSemanticsMenu 是菜单属性处理器的前端半边：把绑定投影成构建管线
 * asset_build_m1 `MenuRegistrationSpec` 的 `semantics.menu`（camelCase），
 * fail-closed；只读本绑定块，不触碰其它 schema 段。后端由 scene-semantics
 * 阶段消费（semantics.menu 为空时该阶段整体短路，菜单编辑不影响其他构建流程）。
 */

/** 一个菜单编辑节点 = 一个目标菜单容器。 */
export interface WorkbenchMenuEntry {
  /** 稳定 id：画布节点/连线 key，与文档一起持久化。 */
  id: string;
  dimension: MenuDimension;
  /** 目标菜单容器 instance（大写十六进制，无 0x 前缀）；空串 = 未选。 */
  categoryInstance: string;
}

/** 菜单绑定：菜单项身份 + 目标容器列表。 */
export interface WorkbenchMenuBinding {
  enabled: boolean;
  /** 菜单显示名（构建期 Menu2 身份的唯一 title，必填）。 */
  label: string;
  description: string;
  /** 面板内排序（对齐原版地标 ~11500）；null = 自动。 */
  uiPosition: number | null;
  entries: WorkbenchMenuEntry[];
}

/** 菜单目标目录：城市 = CATEGORY_MODEL 14 根分类；城市发展 = development-menus.json 9 产业菜单。 */
export interface MenuTargetOption {
  dimension: MenuDimension;
  /** 游戏容器 instance（大写十六进制）。 */
  id: string;
  /** i18n key（t() 直接可用）。 */
  labelKey: string;
}

const CITY_MENU_TARGETS: MenuTargetOption[] = (
  [
    ["AC12C904", "roads"],
    ["3193C0BE", "region"],
    ["8E8B4E10", "power"],
    ["9E3C3DFA", "water"],
    ["CD1DDC35", "sewage"],
    ["674129FD", "government"],
    ["7CB23A4E", "garbage"],
    ["9FAC79C9", "fire"],
    ["DB2D51A3", "health"],
    ["1159ECD7", "police"],
    ["95A9D6FD", "education"],
    ["CBC27856", "transit"],
    ["899BEA70", "park"],
    ["9B7C811F", "disaster"],
  ] as const
).map(([id, key]) => ({
  dimension: "city" as const,
  id,
  labelKey: `studio.assetWorkbench.menuTargets.${key}`,
}));

const DEVELOPMENT_MENU_TARGETS: MenuTargetOption[] = developmentMenus.map(
  (menu) => ({
    dimension: "development" as const,
    id: menu.id,
    labelKey: `studio.workbench.developmentMenus.${menu.key}`,
  }),
);

export const MENU_TARGETS: MenuTargetOption[] = [
  ...CITY_MENU_TARGETS,
  ...DEVELOPMENT_MENU_TARGETS,
];

export function menuTargetsFor(dimension: MenuDimension): MenuTargetOption[] {
  return dimension === "city" ? CITY_MENU_TARGETS : DEVELOPMENT_MENU_TARGETS;
}

/** 目标目录里查 labelKey；未知 instance 返回 null（画布显示原始 id）。 */
export function menuTargetLabelKey(
  dimension: MenuDimension,
  categoryInstance: string,
): string | null {
  return (
    MENU_TARGETS.find(
      (option) =>
        option.dimension === dimension &&
        option.id === categoryInstance.toUpperCase(),
    )?.labelKey ?? null
  );
}

export function emptyMenuBinding(): WorkbenchMenuBinding {
  return { enabled: false, label: "", description: "", uiPosition: null, entries: [] };
}

/** menu-N 递增 id（取现有后缀最大值 +1，与数组顺序无关）。 */
export function newMenuEntryId(existing: WorkbenchMenuEntry[]): string {
  let max = 0;
  for (const entry of existing) {
    const match = /^menu-(\d+)$/.exec(entry.id);
    if (match) max = Math.max(max, Number(match[1]));
  }
  return `menu-${max + 1}`;
}

export function newMenuEntry(
  existing: WorkbenchMenuEntry[],
  dimension: MenuDimension = "city",
): WorkbenchMenuEntry {
  return { id: newMenuEntryId(existing), dimension, categoryInstance: "" };
}

function normalizeEntry(raw: unknown): WorkbenchMenuEntry | null {
  if (typeof raw !== "object" || raw === null) return null;
  const record = raw as Record<string, unknown>;
  const dimension: MenuDimension = record.dimension === "development" ? "development" : "city";
  const categoryInstance =
    typeof record.categoryInstance === "string" ? record.categoryInstance.trim() : "";
  const id = typeof record.id === "string" && record.id.trim() ? record.id.trim() : newMenuEntryId([]);
  return { id, dimension, categoryInstance: categoryInstance.toUpperCase() };
}

/**
 * 文档 → 绑定（恢复/Monaco 回读共用）。防御一切脏数据；旧形态
 * `{enabled, label, category}`（自由文本分类）迁移为单条 city entry，
 * 分类置空待选。
 */
export function normalizeMenuBinding(raw: unknown): WorkbenchMenuBinding {
  const binding = emptyMenuBinding();
  if (typeof raw !== "object" || raw === null) return binding;
  const record = raw as Record<string, unknown>;
  binding.enabled = record.enabled === true;
  if (typeof record.label === "string") binding.label = record.label;
  if (typeof record.description === "string") binding.description = record.description;
  if (typeof record.uiPosition === "number" && Number.isFinite(record.uiPosition))
    binding.uiPosition = record.uiPosition;
  if (Array.isArray(record.entries)) {
    const seen = new Set<string>();
    for (const rawEntry of record.entries) {
      const entry = normalizeEntry(rawEntry);
      if (!entry || seen.has(entry.id)) continue;
      seen.add(entry.id);
      binding.entries.push(entry);
    }
  } else if (binding.enabled || binding.label) {
    // 旧形态迁移：有内容才生成一条，避免死文档凭空长出节点。
    binding.entries.push({ id: "menu-1", dimension: "city", categoryInstance: "" });
  }
  return binding;
}

/** 启用状态下阻碍生成 semantics.menu 的原因列表（fail-closed）。 */
export function menuBindingIssues(binding: WorkbenchMenuBinding): string[] {
  if (!binding.enabled) return [];
  const issues: string[] = [];
  if (!binding.label.trim()) issues.push("menuLabelRequired");
  if (binding.entries.length === 0) issues.push("menuEntryRequired");
  for (const entry of binding.entries)
    if (!entry.categoryInstance) issues.push(`menuCategoryRequired:${entry.id}`);
  return issues;
}

export interface SemanticsMenuDerivation {
  /** null = 不生成（reasons 说明原因；未启用时 reasons 为空且不算错误）。 */
  values: Record<string, unknown> | null;
  reasons: string[];
}

/**
 * 菜单属性处理器（前端半边）：绑定 → `semantics.menu`。
 * 字段与 asset_build_m1 MenuRegistrationSpec（camelCase, deny_unknown_fields）
 * 对齐；categoryInstances 是本次新增的多容器数组字段。仅当有地块参照
 * （reference.lot）时构建侧才会接受——该门槛由 plan.rs 强制，这里只投影菜单块。
 */
export function deriveSemanticsMenu(
  binding: WorkbenchMenuBinding,
): SemanticsMenuDerivation {
  if (!binding.enabled) return { values: null, reasons: [] };
  const reasons = menuBindingIssues(binding);
  if (reasons.length) return { values: null, reasons };
  const values: Record<string, unknown> = {
    title: binding.label.trim(),
    categoryInstances: binding.entries.map((entry) => `0x${entry.categoryInstance}`),
  };
  const description = binding.description.trim();
  if (description) values.description = description;
  if (typeof binding.uiPosition === "number") values.uiPosition = binding.uiPosition;
  return { values, reasons: [] };
}
