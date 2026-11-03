import type { EffectAnnotation, ResourceAnnotation } from "@/api/tauri";

/**
 * 笔记工作区纯逻辑层（2026-10-28 重构）：批注按分类分组、关键词筛选、
 * 排名排序与 TGI 文案。不含 Vue/IO 状态；视图层见 notes.vue 与
 * components/NotesTopicRail.vue。（2026-11-04 增补 effect 语义批注的
 * 按 tag 分组——Property / Effect 双数据源共用同一 rail/筛选骨架。）
 */

/** 一个分类的批注组（items 已按组内原顺序保留）。资源批注按 topic 分组，
 * effect 批注按语义 tag 分组——分组键同为 string，rail 直接复用。 */
export interface TopicGroup<T = ResourceAnnotation> {
  topic: string;
  items: T[];
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

// ---------- effect 语义批注（Effect tab 数据源） ----------

function effectTgiKeyOf(item: EffectAnnotation): string {
  return `${item.typeId}:${item.groupId}:${item.instance}`;
}

/** effect 完整 TGI 文案（复制/悬停用）。 */
export function effectTgiTextOf(item: EffectAnnotation): string {
  return `${hex(item.typeId)}:${hex(item.groupId)}:${hex(item.instance)}`;
}

/** effect TGI instance 段（行内展示用）。 */
export function effectTgiInstanceOf(item: EffectAnnotation): string {
  return hex(item.instance);
}

/** 去重效果数（同一 effect TGI 可有多条 tag/批注）。 */
export function countEffects(items: EffectAnnotation[]): number {
  return new Set(items.map(effectTgiKeyOf)).size;
}

/** 按语义 tag 分组：批注数降序、同名按字典序（与 groupByTopic 同口径）。 */
export function groupEffectAnnotationsByTag(
  items: EffectAnnotation[],
): TopicGroup<EffectAnnotation>[] {
  const map = new Map<string, EffectAnnotation[]>();
  for (const item of items) {
    const list = map.get(item.tag) ?? [];
    list.push(item);
    map.set(item.tag, list);
  }
  return [...map.entries()]
    .map(([topic, groupItems]) => ({ topic, items: groupItems }))
    .sort(
      (a, b) =>
        b.items.length - a.items.length || a.topic.localeCompare(b.topic),
    );
}

/** effect 批注关键词匹配：标签 / 正文 / 完整 TGI（大小写不敏感子串）。 */
export function matchEffectAnnotation(
  item: EffectAnnotation,
  query: string,
): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  const haystack = [item.tag, item.note, effectTgiTextOf(item)];
  return haystack.some((field) => field.toLowerCase().includes(needle));
}

/** 筛选后的 effect 分组：组内逐条过滤，无命中的组整组剔除。 */
export function filterEffectGroups(
  groups: TopicGroup<EffectAnnotation>[],
  query: string,
): TopicGroup<EffectAnnotation>[] {
  if (!query.trim()) return groups;
  return groups
    .map((group) => ({
      topic: group.topic,
      items: group.items.filter((item) => matchEffectAnnotation(item, query)),
    }))
    .filter((group) => group.items.length > 0);
}
