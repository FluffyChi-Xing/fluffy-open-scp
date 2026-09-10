<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { AnnotationTopicStat } from "@/api/tauri";

/**
 * 分类批注统计条（lieflat-charts · Basics F5 Tick Rows 语法的组件适配）：
 * 横排胶囊条、长度∝批注数、条端数值、左侧类目标签；明度梯按排名递减
 * （「明度即数据」，前 4 名用前景色系、其后转灰），超过 8 类合并为
 * 「其他」。颜色取应用 design token，浅/深主题各自成立。
 */
const props = defineProps<{ stats: AnnotationTopicStat[] }>();
const { t } = useI18n();

interface Row {
  label: string;
  value: number;
  extra: string;
}

const MAX_ROWS = 8;
const rows = computed<Row[]>(() => {
  const sorted = [...props.stats].sort((a, b) => b.annotationCount - a.annotationCount);
  const map = (entry: AnnotationTopicStat): Row => ({
    label: entry.topic,
    value: entry.annotationCount,
    extra: t("notes.chartRowExtra", { resources: entry.resourceCount }),
  });
  if (sorted.length <= MAX_ROWS) return sorted.map(map);
  const head = sorted.slice(0, MAX_ROWS - 1).map(map);
  const rest = sorted.slice(MAX_ROWS - 1);
  head.push({
    label: t("notes.chartOther"),
    value: rest.reduce((sum, entry) => sum + entry.annotationCount, 0),
    extra: t("notes.chartRowExtra", { resources: rest.length }),
  });
  return head;
});

const maxValue = computed(() => Math.max(1, ...rows.value.map((row) => row.value)));
function barWidth(value: number): number {
  return (value / maxValue.value) * 100;
}
function barTone(index: number): string {
  if (index < 1) return "var(--foreground)";
  if (index < 3) return "color-mix(in srgb, var(--foreground) 62%, var(--surface))";
  if (index < 5) return "color-mix(in srgb, var(--foreground) 40%, var(--surface))";
  return "color-mix(in srgb, var(--foreground) 26%, var(--surface))";
}
</script>

<template>
  <figure class="topic-bars">
    <figcaption class="topic-bars-head">
      <h2>{{ $t("notes.chartTitle") }}</h2>
      <p class="topic-bars-sub">
        {{ $t("notes.chartSubtitle") }} · {{ $t("notes.chartLegend") }}
      </p>
    </figcaption>
    <ul class="topic-rows">
      <li v-for="(row, index) in rows" :key="row.label" class="topic-row">
        <span class="topic-row-label" :title="row.label">{{ row.label }}</span>
        <span class="topic-row-track">
          <span
            class="topic-row-bar"
            :style="{ width: `${barWidth(row.value)}%`, background: barTone(index) }"
          />
        </span>
        <span class="topic-row-value">{{ row.value }}</span>
        <span class="topic-row-extra">{{ row.extra }}</span>
      </li>
    </ul>
    <p class="topic-bars-src">{{ $t("notes.chartSource") }}</p>
  </figure>
</template>

<style scoped>
.topic-bars {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 24px;
  display: grid;
  gap: 12px;
  margin: 0;
  padding: 18px 20px 14px;
}
.topic-bars-head h2 {
  font-size: 14px;
  margin: 0 0 3px;
}
.topic-bars-sub {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
}
.topic-rows {
  display: grid;
  gap: 10px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.topic-row {
  align-items: center;
  display: flex;
  gap: 10px;
}
.topic-row-label {
  color: var(--foreground);
  flex: none;
  font-size: 12px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: 130px;
}
.topic-row-track {
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
  border-radius: 999px;
  display: block;
  flex: 1;
  height: 12px;
  overflow: hidden;
}
.topic-row-bar {
  border-radius: 999px;
  display: block;
  height: 100%;
  min-width: 10px;
  transition: width 480ms cubic-bezier(0.22, 1, 0.36, 1);
}
.topic-row-value {
  color: var(--foreground);
  flex: none;
  font-size: 12px;
  font-weight: 800;
  font-variant-numeric: tabular-nums;
  text-align: right;
  width: 34px;
}
.topic-row-extra {
  color: var(--muted-foreground);
  flex: none;
  font-size: 10.5px;
  width: 110px;
}
.topic-bars-src {
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.08em;
  margin: 0;
  text-transform: uppercase;
}
@media (prefers-reduced-motion: reduce) {
  .topic-row-bar {
    transition: none;
  }
}
</style>
