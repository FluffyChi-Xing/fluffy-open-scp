<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import TopicBars from "@/components/charts/TopicBars.vue";
import { isTauri, tauriApi } from "@/api";
import type { AnnotationTopicStat, ResourceAnnotation } from "@/api/tauri";
import { useToast } from "@/composables/useToast";

/**
 * 笔记工作区：按分类 topic 汇总的资产批注仪表盘——统计图（lieflat
 * Tick Rows 语法）+ 分组列表（TGI 快捷复制），服务模组作者的结构化
 * 资产管理。
 */
const { t } = useI18n();
const toast = useToast();

const annotations = ref<ResourceAnnotation[]>([]);
const stats = ref<AnnotationTopicStat[]>([]);
const loading = ref(false);

async function load() {
  if (!isTauri()) return;
  loading.value = true;
  try {
    [annotations.value, stats.value] = await Promise.all([
      tauriApi.annotations.list(),
      tauriApi.annotations.topicStats(),
    ]);
  } catch {
    annotations.value = [];
    stats.value = [];
  } finally {
    loading.value = false;
  }
}
onMounted(load);

const grouped = computed(() => {
  const map = new Map<string, ResourceAnnotation[]>();
  for (const item of annotations.value) {
    const list = map.get(item.topic) ?? [];
    list.push(item);
    map.set(item.topic, list);
  }
  return map;
});
const topics = computed(() =>
  [...grouped.value.entries()].sort(
    (a, b) => b[1].length - a[1].length || a[0].localeCompare(b[0]),
  ),
);
const totalAnnotations = computed(() => annotations.value.length);
const totalResources = computed(
  () => new Set(annotations.value.map(tgiKeyOf)).size,
);

function tgiKeyOf(item: ResourceAnnotation): string {
  return `${item.typeId}:${item.groupId}:${item.instance}`;
}
function tgiText(item: ResourceAnnotation): string {
  return `${hex(item.typeId)}:${hex(item.groupId)}:${hex(item.instance)}`;
}
function hex(value: number): string {
  return value.toString(16).padStart(8, "0");
}
async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(t("package.commandCopied"));
  } catch {
    toast.error(t("package.commandCopyFailed"));
  }
}
function copyTopic(topic: string) {
  const lines = (grouped.value.get(topic) ?? [])
    .map((item) => `${item.title}  ${tgiText(item)}`);
  void copy(lines.join("\n"));
}
async function remove(id: number) {
  try {
    await tauriApi.annotations.remove(id);
    await load();
  } catch {
    toast.error(t("notes.deleteFailed"));
  }
}
function packageName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}
</script>

<template>
  <section class="notes-page">
    <header class="notes-page-head">
      <div>
        <h1>{{ $t("notes.workspaceTitle") }}</h1>
        <p class="notes-subtitle">
          {{ $t("notes.workspaceSubtitle", {
            topics: topics.length,
            annotations: totalAnnotations,
            resources: totalResources,
          }) }}
        </p>
      </div>
      <button class="notes-refresh" type="button" :disabled="loading" @click="load">
        <FIcon :name="loading ? 'Loader2' : 'RotateCw'" :size="14" aria-label="" />
        {{ $t("notes.refresh") }}
      </button>
    </header>

    <FSpinner v-if="loading" size="sm" :label="$t('common.loading')" />

    <p v-else-if="!topics.length" class="notes-empty">
      {{ $t("notes.workspaceEmpty") }}
    </p>

    <template v-else>
      <TopicBars :stats="stats" class="notes-chart" />

      <section v-for="[topic, items] in topics" :key="topic" class="topic-card">
        <header class="topic-head">
          <span class="topic-chip">{{ topic }}</span>
          <span class="topic-count">
            {{ $t("notes.topicMeta", { annotations: items.length,
              resources: new Set(items.map(tgiKeyOf)).size }) }}
          </span>
          <button
            class="topic-copy"
            type="button"
            :title="$t('notes.copyTopicTgi')"
            @click="copyTopic(topic)"
          >
            <FIcon name="Copy" :size="13" aria-label="" />
            {{ $t("notes.copyTopicTgi") }}
          </button>
        </header>
        <ul class="topic-list">
          <li v-for="item in items" :key="item.id" class="topic-item">
            <div class="topic-item-main">
              <strong>{{ item.title }}</strong>
              <code class="topic-tgi">{{ tgiText(item) }}</code>
              <span class="topic-pkg">{{ packageName(item.packagePath) }}</span>
            </div>
            <div class="topic-item-actions">
              <button
                type="button"
                :title="$t('notes.copyTgi')"
                @click="copy(tgiText(item))"
              >
                <FIcon name="Copy" :size="13" aria-label="" />
              </button>
              <button
                type="button"
                :title="$t('common.delete')"
                @click="remove(item.id)"
              >
                <FIcon name="Trash2" :size="13" aria-label="" />
              </button>
            </div>
          </li>
        </ul>
      </section>
    </template>
  </section>
</template>

<style scoped>
.notes-page {
  display: grid;
  gap: 18px;
  margin: 0 auto;
  max-width: 960px;
  padding: 18px 20px 40px;
}
.notes-page-head {
  align-items: center;
  display: flex;
  justify-content: space-between;
}
.notes-page-head h1 {
  font-size: 18px;
  margin: 0 0 4px;
}
.notes-subtitle {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
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
.topic-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: grid;
  gap: 10px;
  padding: 14px 16px;
}
.topic-head {
  align-items: center;
  display: flex;
  gap: 10px;
}
.topic-chip {
  background: var(--accent);
  border-radius: 999px;
  font-size: 12px;
  font-weight: 650;
  padding: 3px 12px;
}
.topic-count {
  color: var(--muted-foreground);
  font-size: 11px;
}
.topic-copy {
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
.topic-copy:hover {
  background: var(--accent);
}
.topic-list {
  display: grid;
  gap: 6px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.topic-item {
  align-items: center;
  border-top: 1px dashed var(--border);
  display: flex;
  gap: 10px;
  padding: 8px 0 0;
}
.topic-item-main {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  min-width: 0;
}
.topic-item-main strong {
  font-size: 12px;
}
.topic-tgi {
  color: var(--muted-foreground);
  font: 11px ui-monospace, Consolas, monospace;
}
.topic-pkg {
  color: var(--muted-foreground);
  font-size: 11px;
}
.topic-item-actions {
  display: inline-flex;
  gap: 4px;
  margin-left: auto;
}
.topic-item-actions button {
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
.topic-item-actions button:hover {
  background: var(--accent);
  color: var(--foreground);
}
</style>
