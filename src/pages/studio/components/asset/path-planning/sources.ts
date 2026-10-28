import type { PathSourceSummary } from "@/api/tauri";

/**
 * 路径来源选择（目标建筑卡）的纯函数层，2026-10-28。
 * 获取（list_path_sources）之外的筛选/分页/展示推导全部在这里，
 * 与 Vue 组件解耦，可独立测试与复用。
 */

export interface SourcePage<T> {
  items: T[];
  page: number;
  pageCount: number;
  total: number;
}

/** 关键字过滤：命中注册表名或 TGI instance hex（大小写无关，容忍 0x 前缀）。 */
export function filterPathSources(
  rows: PathSourceSummary[],
  keyword: string,
): PathSourceSummary[] {
  const needle = keyword.trim().toLowerCase().replace(/^0x/, "");
  if (!needle) return rows;
  return rows.filter(
    (row) =>
      row.name.toLowerCase().includes(needle) ||
      row.instance.toString(16).includes(needle),
  );
}

/** 分页（1-based）；越界页自动钳制到最后一页。 */
export function paginate<T>(rows: T[], page: number, pageSize: number): SourcePage<T> {
  const size = Math.max(1, pageSize);
  const pageCount = Math.max(1, Math.ceil(rows.length / size));
  const safePage = Math.min(Math.max(1, page), pageCount);
  const start = (safePage - 1) * size;
  return {
    items: rows.slice(start, start + size),
    page: safePage,
    pageCount,
    total: rows.length,
  };
}

/** 筛选 + 分页一步到位（组件 computed 直接消费）。 */
export function selectPathSourcePage(
  rows: PathSourceSummary[],
  keyword: string,
  page: number,
  pageSize: number,
): SourcePage<PathSourceSummary> {
  return paginate(filterPathSources(rows, keyword), page, pageSize);
}

/** 来源显示名：注册表名优先，无名回退 instance hex。 */
export function sourceDisplayLabel(source: {
  instance: number;
  label?: string;
}): string {
  return source.label?.trim() || `0x${source.instance.toString(16).toUpperCase()}`;
}

/** entry 展示：hex 大写（与 lot 卡 TGI 展示一致）。 */
export function entryDisplayLabel(entry: string | null): string {
  if (!entry) return "";
  return `0x${entry.replace(/^0x/i, "").toUpperCase()}`;
}
