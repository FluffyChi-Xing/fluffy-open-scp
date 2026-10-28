import type { ResourceAnnotation } from "@/api/tauri";

/**
 * 笔记工作区纯逻辑层（2026-10-28 重构）：批注按分类分组、关键词筛选、
 * 排名排序与 TGI 文案。不含 Vue/IO 状态；视图层见 notes.vue 与
 * components/NotesTopicRail.vue。
 */

/** 一个分类的批注组（items 已按组内原顺序保留）。 */
export interface TopicGroup {
  topic: string;
  items: ResourceAnnotation[];
}

export function hex(value: number): string {
  return value.toString(16).padStart(8, "0");
}

export function tgiKeyOf(item: ResourceAnnotation): string {
  return `${item.typeId}:${item.groupId}:${item.instance}`;
}

/** 完整 TGI 文案（复制/悬停用）。 */
export function tgiTextOf(item: ResourceAnnotation): string {
  return `${hex(item.typeId)}:${hex(item.groupId)}:${hex(item.instance)}`;
}

/** TGI instance 段（行内展示用：typeId/group 对同一资源类恒同，是常驻重复）。 */
export function tgiInstanceOf(item: ResourceAnnotation): string {
  return hex(item.instance);
}

export function packageName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** 按批注数降序、同名按字典序的分组。 */
export function groupByTopic(annotations: ResourceAnnotation[]): TopicGroup[] {
  const map = new Map<string, ResourceAnnotation[]>();
  for (const item of annotations) {
    const list = map.get(item.topic) ?? [];
    list.push(item);
    map.set(item.topic, list);
  }
  return [...map.entries()]
    .map(([topic, items]) => ({ topic, items }))
    .sort(
      (a, b) =>
        b.items.length - a.items.length || a.topic.localeCompare(b.topic),
    );
}

/** 关键词匹配：标题 / 分类 / 正文 / 完整 TGI / 包名（大小写不敏感子串）。 */
export function matchAnnotation(
  item: ResourceAnnotation,
  query: string,
): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  const haystack = [
    item.title,
    item.topic,
    item.content,
    tgiTextOf(item),
    item.packagePath,
  ];
  return haystack.some((field) => field.toLowerCase().includes(needle));
}

/** 筛选后的分组：组内逐条过滤，无命中的组整组剔除（保持原排序）。 */
export function filterGroups(
  groups: TopicGroup[],
  query: string,
): TopicGroup[] {
  if (!query.trim()) return groups;
  return groups
    .map((group) => ({
      topic: group.topic,
      items: group.items.filter((item) => matchAnnotation(item, query)),
    }))
    .filter((group) => group.items.length > 0);
}

/** 组内全部批注同包时返回包文件名（表头去重展示；按 basename 口径，
 * 与行内展示一致），否则 null。 */
export function uniformPackageName(items: ResourceAnnotation[]): string | null {
  if (!items.length) return null;
  const first = packageName(items[0].packagePath);
  return items.every((item) => packageName(item.packagePath) === first)
    ? first
    : null;
}

/** 去重资源数（同一 TGI 可能有多条批注）。 */
export function countResources(items: ResourceAnnotation[]): number {
  return new Set(items.map(tgiKeyOf)).size;
}
