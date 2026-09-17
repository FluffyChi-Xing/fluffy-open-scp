<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ModProjectStats } from "@/api/tauri";

/**
 * 项目统计仪表盘。
 *
 * 【项目硬约定】统计图必须用 lieflat-charts skill 方案手写 SVG/CSS 实现，
 * 禁止 echarts/chart.js。遵循其语法：Mono 明度即数据、结论式标题、
 * 卡片四件套（结论标题 + 副标题图例 + 图 + 来源行）、柱不断轴、胶囊圆角。
 */
const props = defineProps<{ stats: ModProjectStats | null }>();
const { t, locale } = useI18n();

/** 明度即数据：主角 = primary，次要 = 灰阶。 */
const HERO = "var(--primary)";
const MUTED = "var(--muted-foreground)";

function formatBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${bytes} B`;
}

const kpis = computed(() => [
  {
    label: t("studio.dashboard.kpiTotal"),
    value: String(props.stats?.total ?? 0),
  },
  {
    label: t("studio.dashboard.kpiGroups"),
    value: String(
      props.stats?.groupCounts.filter((entry) => entry.count > 0).length ?? 0,
    ),
  },
  {
    label: t("studio.dashboard.kpiDisk"),
    value: formatBytes(props.stats?.diskBytes ?? 0),
  },
]);

const hasData = computed(() => (props.stats?.total ?? 0) > 0);

/** 分组分布：横排 tick rows，长度∝数量，峰值主角用主色。 */
const groupRows = computed(() => {
  const entries = [...(props.stats?.groupCounts ?? [])].sort(
    (a, b) => b.count - a.count,
  );
  const max = Math.max(1, ...entries.map((entry) => entry.count));
  return entries.map((entry) => ({
    label:
      entry.groupId === null || entry.groupId === undefined
        ? t("studio.dashboard.ungrouped")
        : (entry.name ?? t("studio.dashboard.ungrouped")),
    count: entry.count,
    ratio: entry.count / max,
    hero: entry.count === max,
  }));
});

/** 近 30 天：每日双柱（创建/更新），高度∝数量，绝不断轴。 */
const activity = computed(() => {
  const days = props.stats?.recentActivity ?? [];
  const max = Math.max(1, ...days.flatMap((day) => [day.created, day.updated]));
  const formatter = new Intl.DateTimeFormat(locale.value, {
    month: "numeric",
    day: "numeric",
  });
  const columns = days.map((day) => ({
    label: formatter.format(new Date(day.dayStartMs)),
    created: day.created,
    updated: day.updated,
    createdH: `${(day.created / max) * 100}%`,
    updatedH: `${(day.updated / max) * 100}%`,
  }));
  // 标签稀疏化：最多约 6 个刻度；末位（最新一天）始终显示
  const tickEvery = Math.max(1, Math.ceil(columns.length / 6));
  const ticks = columns
    .map((column, index) => ({ ...column, show: index % tickEvery === 0 }))
    .map((tick, index, all) => ({
      ...tick,
      show: tick.show || index === all.length - 1,
    }));
  return { columns, ticks, max };
});

/** 状态构成 waffle：一个方块 = 一个项目，明度区分状态。 */
const waffle = computed(() => {
  const palette: Record<string, string> = {
    active: HERO,
    released: MUTED,
    archived: "color-mix(in srgb, var(--muted-foreground) 40%, transparent)",
  };
  const cells: { status: string; color: string }[] = [];
  for (const entry of props.stats?.statusCounts ?? []) {
    for (let index = 0; index < entry.count; index += 1) {
      cells.push({
        status: entry.status,
        color: palette[entry.status] ?? MUTED,
      });
    }
  }
  return cells;
});

function statusLabel(status: string): string {
  const key = `studio.dashboard.status.${status}`;
  const label = t(key);
  return label === key ? status : label;
}
</script>

<template>
  <section class="dashboard" :aria-label="$t('studio.dashboard.title')">
    <div class="kpi-row">
      <div v-for="kpi in kpis" :key="kpi.label" class="kpi-card">
        <span class="kpi-value">{{ kpi.value }}</span>
        <span class="kpi-label">{{ kpi.label }}</span>
      </div>
    </div>
    <div v-if="hasData" class="chart-grid">
      <!-- 分组分布：tick rows -->
      <article class="chart-card">
        <h2>{{ $t("studio.dashboard.groupDistTitle") }}</h2>
        <p class="sub">{{ $t("studio.dashboard.groupDistSub") }}</p>
        <div class="rungs" role="img">
          <div v-for="row in groupRows" :key="row.label" class="rung">
            <span class="rung-label" :title="row.label">{{ row.label }}</span>
            <span class="rung-track">
              <span
                class="rung-bar"
                :class="{ hero: row.hero }"
                :style="{ width: `${row.ratio * 100}%` }"
              ></span>
            </span>
            <span class="rung-value">{{ row.count }}</span>
          </div>
        </div>
        <p class="source">{{ $t("studio.dashboard.source") }}</p>
      </article>

      <!-- 近 30 天：双序列细柱 -->
      <article class="chart-card">
        <h2>{{ $t("studio.dashboard.activityTitle") }}</h2>
        <p class="sub">
          <span class="legend-dot hero" aria-hidden="true"></span
          >{{ $t("studio.dashboard.created") }}
          <span class="legend-dot muted" aria-hidden="true"></span
          >{{ $t("studio.dashboard.updated") }}
        </p>
        <div class="columns" role="img">
          <div
            v-for="(column, index) in activity.columns"
            :key="index"
            class="column"
            :title="`${column.label} · ${$t('studio.dashboard.created')} ${column.created} / ${$t('studio.dashboard.updated')} ${column.updated}`"
          >
            <span class="pair">
              <i class="bar created" :style="{ height: column.createdH }"></i
              ><i class="bar updated" :style="{ height: column.updatedH }"></i>
            </span>
          </div>
        </div>
        <div class="axis">
          <span
            v-for="tick in activity.ticks"
            :key="tick.label"
            class="axis-label"
            >{{ tick.show ? tick.label : "" }}</span
          >
        </div>
        <p class="source">{{ $t("studio.dashboard.source") }}</p>
      </article>

      <!-- 状态构成 waffle -->
      <article class="chart-card">
        <h2>{{ $t("studio.dashboard.statusTitle") }}</h2>
        <p class="sub">{{ $t("studio.dashboard.statusSub") }}</p>
        <div
          class="waffle"
          role="img"
          :aria-label="
            waffle.map((cell) => statusLabel(cell.status)).join(', ')
          "
        >
          <span
            v-for="(cell, index) in waffle"
            :key="index"
            class="cell"
            :style="{ background: cell.color }"
            :title="statusLabel(cell.status)"
          ></span>
        </div>
        <p class="status-legend">
          <span
            v-for="entry in stats?.statusCounts ?? []"
            :key="entry.status"
            class="status-item"
          >
            <b>{{ entry.count }}</b> {{ statusLabel(entry.status) }}
          </span>
        </p>
        <p class="source">{{ $t("studio.dashboard.source") }}</p>
      </article>
    </div>
    <p v-else class="dashboard-empty">{{ $t("studio.dashboard.empty") }}</p>
  </section>
</template>

<style scoped>
.dashboard {
  display: grid;
  gap: 14px;
}
.kpi-row {
  display: grid;
  gap: 14px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}
.kpi-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  display: grid;
  gap: 2px;
  padding: 16px 18px;
}
.kpi-value {
  font-size: 22px;
  font-variant-numeric: tabular-nums;
  font-weight: 750;
  letter-spacing: -0.02em;
}
.kpi-label {
  color: var(--muted-foreground);
  font-size: 11px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.chart-grid {
  display: grid;
  gap: 14px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.chart-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  display: grid;
  align-content: start;
  gap: 6px;
  padding: 18px 18px 12px;
}
.chart-card h2 {
  font-size: 14px;
  letter-spacing: -0.01em;
  margin: 0;
}
.sub {
  color: var(--muted-foreground);
  align-items: center;
  display: flex;
  font-size: 11.5px;
  gap: 6px;
  margin: 0;
}
.legend-dot {
  border-radius: 2px;
  display: inline-block;
  height: 9px;
  width: 9px;
}
.legend-dot.hero {
  background: var(--primary);
}
.legend-dot.muted {
  background: var(--muted-foreground);
}
.source {
  color: var(--subtle-foreground);
  font-size: 10px;
  letter-spacing: 0.08em;
  margin: 0;
  text-transform: uppercase;
}
/* 分组分布：tick rows */
.rungs {
  display: grid;
  gap: 10px;
  padding: 10px 0 14px;
}
.rung {
  align-items: center;
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(72px, 120px) minmax(0, 1fr) 24px;
}
.rung-label {
  color: var(--muted-foreground);
  font-size: 11.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rung-track {
  background: color-mix(in srgb, var(--muted-foreground) 10%, transparent);
  border-radius: 999px;
  height: 14px;
  overflow: hidden;
}
.rung-bar {
  background: var(--muted-foreground);
  border-radius: 999px;
  display: block;
  height: 100%;
  transition: width 300ms ease;
}
.rung-bar.hero {
  background: var(--primary);
}
.rung-value {
  color: var(--muted-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  text-align: end;
}
/* 近 30 天：双序列细柱 */
.columns {
  align-items: stretch;
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 2px;
  height: 140px;
  padding: 0 2px;
}
.column {
  align-items: flex-end;
  display: flex;
  flex: 1;
  height: 100%;
  justify-content: center;
}
.pair {
  align-items: flex-end;
  display: flex;
  gap: 2px;
  height: 100%;
}
.bar {
  border-radius: 3px 3px 0 0;
  display: block;
  min-height: 0;
  width: 6px;
}
.bar.created {
  background: var(--primary);
}
.bar.updated {
  background: color-mix(in srgb, var(--muted-foreground) 55%, transparent);
}
.axis {
  color: var(--subtle-foreground);
  display: flex;
  font-size: 10px;
  justify-content: space-between;
  padding: 4px 2px 2px;
}
.axis-label {
  min-width: 0;
}
/* 状态 waffle */
.waffle {
  display: grid;
  gap: 4px;
  grid-template-columns: repeat(auto-fill, minmax(14px, 1fr));
  padding: 12px 0 4px;
}
.cell {
  aspect-ratio: 1;
  border-radius: 3px;
}
.status-legend {
  color: var(--muted-foreground);
  display: flex;
  flex-wrap: wrap;
  font-size: 11.5px;
  gap: 12px;
  margin: 0;
}
.status-item b {
  color: var(--foreground);
  font-variant-numeric: tabular-nums;
}
.dashboard-empty {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  color: var(--muted-foreground);
  font-size: 12.5px;
  padding: 26px 12px;
  text-align: center;
}
@media (max-width: 980px) {
  .chart-grid {
    grid-template-columns: 1fr;
  }
}
</style>
