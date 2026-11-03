<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { isTauri, tauriApi } from "@/api";
import type { EffectAnnotation, ResourceAnnotation } from "@/api/tauri";
import { useToast } from "@/composables/useToast";
import NotesTopicRail from "./components/NotesTopicRail.vue";
import {
  countEffects,
  countResources,
  effectTgiInstanceOf,
  effectTgiTextOf,
  filterEffectGroups,
  filterGroups,
  groupByTopic,
  groupEffectAnnotationsByTag,
  packageName,
  tgiInstanceOf,
  tgiKeyOf,
  tgiTextOf,
  uniformPackageName,
} from "./notes-model";

/**
 * 笔记工作区（2026-10-28 重构）：主从两栏替代纵向卡片堆——左栏分类
 * 导航（行内迷你条即统计图，sticky 常驻），右栏只渲染选中分类的批注
 * 表。页面长度不再随分类数膨胀；正文预览/展开、关键词筛选、悬停操作
 * 与删除确认补齐可用性。纯逻辑见 notes-model.ts。
 * （2026-11-04 增补：左栏顶部 Property / Effect 双页签——Effect 页签
 * 按语义 tag 分组展示 effect 批注及其 TGI 信息。）
 */
const { t, locale } = useI18n();
const toast = useToast();

type ViewMode = "property" | "effect";
const viewMode = ref<ViewMode>("property");

const annotations = ref<ResourceAnnotation[]>([]);
const effectItems = ref<EffectAnnotation[]>([]);
const loading = ref(false);
const query = ref("");
const selectedTopic = ref<string | null>(null);
const selectedTag = ref<string | null>(null);
const expandedId = ref<number | null>(null);

async function load() {
  if (!isTauri()) return;
  loading.value = true;
  try {
    const [list, effectList] = await Promise.all([
      tauriApi.annotations.list(),
      tauriApi.effectAnnotations.list(),
    ]);
    annotations.value = list;
    effectItems.value = effectList;
  } catch {
    annotations.value = [];
    effectItems.value = [];
  } finally {
    loading.value = false;
  }
}
onMounted(load);

const groups = computed(() => groupByTopic(annotations.value));
const effectGroups = computed(() => groupEffectAnnotationsByTag(effectItems.value));
const filteredGroups = computed(() => filterGroups(groups.value, query.value));
const filteredEffectGroups = computed(() =>
  filterEffectGroups(effectGroups.value, query.value),
);
/** 当前页签的 rail 数据源与选中键（模板统一消费）。 */
const railGroups = computed(() =>
  viewMode.value === "property" ? filteredGroups.value : filteredEffectGroups.value,
);
const selectedKey = computed(() =>
  viewMode.value === "property" ? selectedTopic.value : selectedTag.value,
);
function setSelection(topic: string) {
  if (viewMode.value === "property") selectedTopic.value = topic;
  else selectedTag.value = topic;
}
const selectedGroup = computed(
  () => railGroups.value.find((group) => group.topic === selectedKey.value) ?? null,
);

/** 空态文案：数据源为空（按页签区分引导语）或筛选无命中。 */
const emptyText = computed(() => {
  const sourceEmpty =
    viewMode.value === "property"
      ? !groups.value.length
      : !effectGroups.value.length;
  if (sourceEmpty) {
    return viewMode.value === "property"
      ? t("notes.workspaceEmpty")
      : t("effectTags.workspaceEmptyEffects");
  }
  return t("notes.noMatches");
});

/** 筛选后保序：选中分类被过滤掉（或切换页签）时自动落到首个分类。 */
watch(railGroups, (list) => {
  if (!list.some((group) => group.topic === selectedKey.value)) {
    setSelection(list[0]?.topic ?? null);
  }
});

const totalAnnotations = computed(() => annotations.value.length);
const totalResources = computed(
  () => new Set(annotations.value.map(tgiKeyOf)).size,
);
const totalEffectTags = computed(() => effectGroups.value.length);
const totalEffects = computed(() => countEffects(effectItems.value));

/** 包名去重按全量组判定（筛选子集不应改变表头口径）。 */
const uniformPkg = computed(() => {
  if (viewMode.value !== "property") return null;
  const full = groups.value.find((group) => group.topic === selectedKey.value);
  return full ? uniformPackageName(full.items) : null;
});
const selectedResourceCount = computed(() =>
  viewMode.value === "property" && selectedGroup.value
    ? countResources(selectedGroup.value.items as ResourceAnnotation[])
    : 0,
);
const selectedEffectCount = computed(() =>
  viewMode.value === "effect" && selectedGroup.value
    ? countEffects(selectedGroup.value.items as EffectAnnotation[])
    : 0,
);

async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(t("package.commandCopied"));
  } catch {
    toast.error(t("package.commandCopyFailed"));
  }
}
/** 复制整个分类/标签的 TGI（全量口径，与筛选无关）。 */
function copySelectedTgi() {
  const source =
    viewMode.value === "property"
      ? groups.value.find((group) => group.topic === selectedKey.value)?.items
      : effectGroups.value.find((group) => group.topic === selectedKey.value)
          ?.items;
  if (!source) return;
  const lines = viewMode.value === "property"
    ? (source as ResourceAnnotation[]).map(
        (item) => `${item.title}  ${tgiTextOf(item)}`,
      )
    : (source as EffectAnnotation[]).map(
        (item) => `${item.tag}  ${effectTgiTextOf(item)}`,
      );
  void copy(lines.join("\n"));
}
async function remove(item: ResourceAnnotation) {
  if (!window.confirm(t("notes.deleteConfirm"))) return;
  try {
    await tauriApi.annotations.remove(item.id);
    await load();
  } catch {
    toast.error(t("notes.deleteFailed"));
  }
}
async function removeEffect(item: EffectAnnotation) {
  if (!window.confirm(t("notes.deleteConfirm"))) return;
  try {
    await tauriApi.effectAnnotations.remove(item.id);
    await load();
  } catch {
    toast.error(t("effectTags.deleteFailed"));
  }
}
function toggleExpanded(id: number) {
  expandedId.value = expandedId.value === id ? null : id;
}
function formatTime(ts: number): string {
  return new Intl.DateTimeFormat(locale.value, {
    dateStyle: "short",
    timeStyle: "medium",
  }).format(new Date(ts));
}
function sourceTgiTextOf(item: EffectAnnotation): string {
  if (item.sourceTypeId === null) return "";
  const hex = (value: number) => value.toString(16).padStart(8, "0");
  return `${hex(item.sourceTypeId)}:${hex(item.sourceGroupId ?? 0)}:${hex(
    item.sourceInstanceId ?? 0,
  )}`;
}
</script>

<template>
  <section class="notes-page">
    <header class="notes-head">
      <div>
        <h1>{{ $t("notes.workspaceTitle") }}</h1>
        <p class="notes-subtitle">
          {{
            viewMode === "property"
              ? $t("notes.workspaceSubtitle", {
                  topics: groups.length,
                  annotations: totalAnnotations,
                  resources: totalResources,
                })
              : $t("effectTags.workspaceSubtitleEffects", {
                  tags: totalEffectTags,
                  annotations: effectItems.length,
                  effects: totalEffects,
                })
          }}
        </p>
      </div>
      <div class="notes-tools">
        <label class="notes-search">
          <FIcon name="Search" :size="13" aria-label="" />
          <input
            v-model="query"
            type="search"
            :placeholder="t('notes.searchPlaceholder')"
            :aria-label="t('notes.searchPlaceholder')"
          />
        </label>
        <button
          class="notes-refresh"
          type="button"
          :disabled="loading"
          @click="load"
        >
          <FIcon
            :name="loading ? 'Loader2' : 'RotateCw'"
            :size="14"
            aria-label=""
          />
          {{ $t("notes.refresh") }}
        </button>
      </div>
    </header>

    <FSpinner v-if="loading" size="sm" :label="$t('common.loading')" />

    <!-- 双页签常驻：空态/无命中只占右栏内容区，切换入口永不消失 -->
    <div v-else class="notes-layout">
      <div class="notes-side">
        <div class="mode-tabs" role="tablist" :aria-label="$t('notes.railLabel')">
          <button
            type="button"
            role="tab"
            :aria-selected="viewMode === 'property'"
            :class="{ active: viewMode === 'property' }"
            @click="viewMode = 'property'"
          >
            {{ $t("effectTags.workspaceTabProperty") }}
          </button>
          <button
            type="button"
            role="tab"
            :aria-selected="viewMode === 'effect'"
            :class="{ active: viewMode === 'effect' }"
            @click="viewMode = 'effect'"
          >
            {{ $t("effectTags.workspaceTabEffect") }}
          </button>
        </div>
        <NotesTopicRail
          v-if="railGroups.length"
          class="notes-rail"
          :groups="railGroups"
          :selected="selectedKey"
          @select="setSelection"
        />
        <p v-else class="rail-empty">{{ emptyText }}</p>
      </div>

      <section v-if="selectedGroup" class="topic-pane">
        <header class="pane-head">
          <span class="pane-chip">{{ selectedGroup.topic }}</span>
          <span class="pane-meta">
            {{
              viewMode === "property"
                ? $t("notes.topicMeta", {
                    annotations: selectedGroup.items.length,
                    resources: selectedResourceCount,
                  })
                : $t("effectTags.countMeta", {
                    annotations: selectedGroup.items.length,
                    effects: selectedEffectCount,
                  })
            }}
          </span>
          <span v-if="uniformPkg" class="pane-pkg" :title="uniformPkg">{{
            uniformPkg
          }}</span>
          <button class="pane-copy" type="button" @click="copySelectedTgi">
            <FIcon name="Copy" :size="13" aria-label="" />
            {{ $t("notes.copyTopicTgi") }}
          </button>
        </header>

        <ul v-if="viewMode === 'effect'" class="note-list">
          <li
            v-for="item in (selectedGroup.items as EffectAnnotation[])"
            :key="item.id"
            class="note-row"
            :class="{ expanded: expandedId === item.id }"
          >
            <div
              class="note-row-main"
              role="button"
              tabindex="0"
              :aria-expanded="expandedId === item.id"
              @click="toggleExpanded(item.id)"
              @keydown.enter.prevent
              @keydown.space.prevent
            >
              <div class="note-title-line">
                <FIcon
                  name="ChevronDown"
                  :size="12"
                  class="note-chevron"
                  aria-label=""
                />
                <FIcon name="Zap" :size="12" class="note-zap" aria-label="" />
                <code
                  class="note-tgi"
                  :title="`${effectTgiTextOf(item)} · ${t('notes.copyTgi')}`"
                  @click.stop="copy(effectTgiTextOf(item))"
                >
                  {{ effectTgiInstanceOf(item) }}
                </code>
                <code
                  v-if="item.sourceTypeId !== null"
                  class="note-tgi note-src"
                  :title="`${t('effectTags.sourceTgiLabel')} ${sourceTgiTextOf(item)}`"
                >
                  {{ sourceTgiTextOf(item).split(":")[2] }}
                </code>
              </div>
              <p v-if="item.note && expandedId !== item.id" class="note-preview">
                {{ item.note }}
              </p>
            </div>
            <div class="note-actions">
              <button
                type="button"
                :title="t('notes.copyTgi')"
                @click.stop="copy(effectTgiTextOf(item))"
              >
                <FIcon name="Copy" :size="13" aria-label="" />
              </button>
              <button
                type="button"
                :title="t('common.delete')"
                @click.stop="removeEffect(item)"
              >
                <FIcon name="Trash2" :size="13" aria-label="" />
              </button>
            </div>
            <div v-if="expandedId === item.id" class="note-body">
              <pre class="note-content">{{
                item.note || $t("notes.noContent")
              }}</pre>
              <p class="note-time">
                <template v-if="item.sourceTypeId !== null">
                  {{ $t("effectTags.sourceTgiLabel") }}
                  {{ sourceTgiTextOf(item) }} ·
                </template>
                {{
                  $t("notes.updatedAt", {
                    time: formatTime(item.updatedAt || item.createdAt),
                  })
                }}
              </p>
            </div>
          </li>
        </ul>

        <ul v-else class="note-list">
          <li
            v-for="item in (selectedGroup.items as ResourceAnnotation[])"
            :key="item.id"
            class="note-row"
            :class="{ expanded: expandedId === item.id }"
          >
            <div
              class="note-row-main"
              role="button"
              tabindex="0"
              :aria-expanded="expandedId === item.id"
              @click="toggleExpanded(item.id)"
              @keydown.enter.prevent
              @keydown.space.prevent
            >
              <div class="note-title-line">
                <FIcon
                  name="ChevronDown"
                  :size="12"
                  class="note-chevron"
                  aria-label=""
                />
                <strong class="note-title">{{ item.title }}</strong>
                <code
                  class="note-tgi"
                  :title="`${tgiTextOf(item)} · ${t('notes.copyTgi')}`"
                  @click.stop="copy(tgiTextOf(item))"
                >
                  {{ tgiInstanceOf(item) }}
                </code>
                <span v-if="!uniformPkg" class="note-pkg">
                  {{ packageName(item.packagePath) }}
                </span>
              </div>
              <p
                v-if="item.content && expandedId !== item.id"
                class="note-preview"
              >
                {{ item.content }}
              </p>
            </div>
            <div class="note-actions">
              <button
                type="button"
                :title="t('notes.copyTgi')"
                @click.stop="copy(tgiTextOf(item))"
              >
                <FIcon name="Copy" :size="13" aria-label="" />
              </button>
              <button
                type="button"
                :title="t('common.delete')"
                @click.stop="remove(item)"
              >
                <FIcon name="Trash2" :size="13" aria-label="" />
              </button>
            </div>
            <div v-if="expandedId === item.id" class="note-body">
              <pre class="note-content">{{
                item.content || $t("notes.noContent")
              }}</pre>
              <p class="note-time">
                {{
                  $t("notes.updatedAt", {
                    time: formatTime(item.updatedAt || item.createdAt),
                  })
                }}
              </p>
            </div>
          </li>
        </ul>
      </section>

      <p v-else class="notes-empty notes-empty-pane">{{ emptyText }}</p>
    </div>
  </section>
</template>

<style scoped>
.notes-page {
  display: grid;
  gap: 16px;
}
.notes-head {
  align-items: flex-end;
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  justify-content: space-between;
}
.notes-head h1 {
  font-size: 18px;
  margin: 0 0 4px;
}
.notes-subtitle {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
}
.notes-tools {
  align-items: center;
  display: flex;
  gap: 8px;
}
.notes-search {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: inline-flex;
  gap: 6px;
  min-height: 30px;
  padding: 0 9px;
}
.notes-search:focus-within {
  border-color: var(--primary);
}
.notes-search input {
  background: transparent;
  border: 0;
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  outline: none;
  width: 208px;
}
.notes-refresh {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  gap: 6px;
  min-height: 30px;
  padding: 0 12px;
}
.notes-refresh:hover {
  background: var(--accent);
}
.notes-empty {
  color: var(--muted-foreground);
  padding: 40px 0;
  text-align: center;
}
/* 空态占据右栏内容位（左栏页签常驻，切换入口不消失） */
.notes-empty-pane {
  align-self: start;
}
.rail-empty {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.6;
  margin: 0;
  padding: 10px 6px 14px;
}

/* ---- 主从两栏：左栏 sticky 常驻，右栏单分类，页面高度有界 ---- */
.notes-layout {
  align-items: start;
  display: grid;
  gap: 14px;
  grid-template-columns: minmax(196px, 236px) minmax(0, 1fr);
}
.notes-side {
  display: grid;
  gap: 8px;
  max-height: calc(100dvh - 108px);
  min-height: 0;
  position: sticky;
  top: 68px;
}
/* Property / Effect 双页签（segmented control） */
.mode-tabs {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: grid;
  gap: 3px;
  grid-template-columns: 1fr 1fr;
  padding: 3px;
}
.mode-tabs button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11.5px;
  justify-content: center;
  min-height: 26px;
  padding: 0 8px;
}
.mode-tabs button:hover {
  color: var(--foreground);
}
.mode-tabs button.active {
  background: var(--accent);
  color: var(--foreground);
  font-weight: 650;
}
.notes-rail {
  min-height: 0;
}

.topic-pane {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: grid;
  gap: 4px;
  padding: 12px 14px;
}
.pane-head {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding-bottom: 8px;
}
.pane-chip {
  background: var(--accent);
  border-radius: 999px;
  font-size: 12px;
  font-weight: 650;
  padding: 3px 12px;
}
.pane-meta {
  color: var(--muted-foreground);
  font-size: 11px;
}
.pane-pkg {
  color: var(--muted-foreground);
  font:
    10.5px ui-monospace,
    Consolas,
    monospace;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pane-copy {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 5px;
  margin-left: auto;
  min-height: 26px;
  padding: 0 9px;
}
.pane-copy:hover {
  background: var(--accent);
}

.note-list {
  display: grid;
  list-style: none;
  margin: 0;
  padding: 0;
}
.note-row {
  align-items: flex-start;
  border-top: 1px solid var(--border);
  display: flex;
  flex-wrap: wrap;
  position: relative;
}
.note-row:first-child {
  border-top: 0;
}
.note-row-main {
  cursor: pointer;
  flex: 1;
  min-width: 0;
  padding: 6px 10px 7px 6px;
}
.note-row-main:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: -2px;
}
.note-title-line {
  align-items: center;
  display: flex;
  gap: 8px;
  min-width: 0;
}
.note-chevron {
  color: var(--muted-foreground);
  flex: none;
  transition: transform 140ms ease;
}
.note-row.expanded .note-chevron {
  transform: rotate(180deg);
}
.note-zap {
  color: var(--primary);
  flex: none;
}
.note-src {
  background: var(--surface-hover);
}
.note-title {
  flex: 0 1 auto;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.note-tgi {
  border-radius: 3px;
  color: var(--muted-foreground);
  cursor: copy;
  flex: none;
  font:
    11px ui-monospace,
    Consolas,
    monospace;
  padding: 1px 4px;
}
.note-tgi:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.note-pkg {
  color: var(--muted-foreground);
  flex: none;
  font-size: 10.5px;
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.note-preview {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 3px 0 0 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.note-actions {
  display: inline-flex;
  gap: 4px;
  opacity: 0;
  padding: 5px 8px 0 0;
  transition: opacity 120ms ease;
}
.note-row:hover .note-actions,
.note-row:focus-within .note-actions {
  opacity: 1;
}
@media (hover: none) {
  .note-actions {
    opacity: 1;
  }
}
.note-actions button {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  min-height: 24px;
  padding: 0 6px;
}
.note-actions button:hover {
  background: var(--accent);
  color: var(--foreground);
}
.note-body {
  flex-basis: 100%;
  padding: 0 44px 9px 26px;
}
.note-content {
  background: var(--surface-elevated);
  border-radius: var(--radius-sm);
  font: inherit;
  font-size: 11.5px;
  line-height: 1.65;
  margin: 0;
  padding: 8px 10px;
  white-space: pre-wrap;
  word-break: break-word;
}
.note-time {
  color: var(--muted-foreground);
  font-size: 10.5px;
  margin: 5px 0 0;
}

@media (max-width: 900px) {
  .notes-layout {
    grid-template-columns: 1fr;
  }
  .notes-side {
    max-height: 38vh;
    position: static;
  }
  .note-body {
    padding-right: 12px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .note-chevron,
  .note-actions {
    transition: none;
  }
}
</style>
