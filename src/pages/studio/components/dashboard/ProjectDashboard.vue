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
 * 活动热力采用 L17 Calendar Heat / GitHub contributions 形态。
 */
const props = defineProps<{ stats: ModProjectStats | null }>();
const { t, locale } = useI18n();

const MS_DAY = 86_400_000;

function formatBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${bytes} B`;
}

function localDayStart(ms: number): number {
  const date = new Date(ms);
  return new Date(
    date.getFullYear(),
    date.getMonth(),
    date.getDate(),
  ).getTime();
}

/** Monday-based weekday index: 0 = Mon … 6 = Sun. */
function mondayIndex(ms: number): number {
  return (new Date(ms).getDay() + 6) % 7;
}

function activityLevel(total: number, max: number): 0 | 1 | 2 | 3 | 4 {
  if (total <= 0 || max <= 0) return 0;
  const ratio = total / max;
  if (ratio <= 0.25) return 1;
  if (ratio <= 0.5) return 2;
  if (ratio <= 0.75) return 3;
  return 4;
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

type HeatCell = {
  key: string;
  inRange: boolean;
  created: number;
  updated: number;
  total: number;
  level: 0 | 1 | 2 | 3 | 4;
  title: string;
};

/**
 * 近 30 天 GitHub contributions 热力：列 = 周，行 = 周一到周日，
 * 明度等级 = 当日创建+更新。下方并入项目状态计数。
 */
const contributions = computed(() => {
  const days = props.stats?.recentActivity ?? [];
  const byDay = new Map<number, { created: number; updated: number }>();
  let activityTotal = 0;
  let max = 0;

  for (const day of days) {
    const key = localDayStart(day.dayStartMs);
    const created = day.created;
    const updated = day.updated;
    const total = created + updated;
    byDay.set(key, { created, updated });
    activityTotal += total;
    if (total > max) max = total;
  }

  if (days.length === 0) {
    return {
      weeks: [] as HeatCell[][],
      monthLabels: [] as { label: string; weekIndex: number }[],
      activityTotal: 0,
      max: 0,
    };
  }

  const timestamps = [...byDay.keys()].sort((a, b) => a - b);
  const rangeStart = timestamps[0]!;
  const rangeEnd = timestamps[timestamps.length - 1]!;

  const gridStart = rangeStart - mondayIndex(rangeStart) * MS_DAY;
  const gridEnd = rangeEnd + (6 - mondayIndex(rangeEnd)) * MS_DAY;

  const dayFormatter = new Intl.DateTimeFormat(locale.value, {
    month: "short",
    day: "numeric",
  });
  const monthFormatter = new Intl.DateTimeFormat(locale.value, {
    month: "short",
  });

  const weeks: HeatCell[][] = [];
  const monthLabels: { label: string; weekIndex: number }[] = [];
  let cursor = gridStart;
  let weekIndex = 0;
  let lastMonth = -1;

  while (cursor <= gridEnd) {
    const week: HeatCell[] = [];
    let monthAnchor: Date | null = null;
    for (let dow = 0; dow < 7; dow += 1) {
      const ms = cursor + dow * MS_DAY;
      const entry = byDay.get(ms);
      const created = entry?.created ?? 0;
      const updated = entry?.updated ?? 0;
      const total = created + updated;
      const inRange = ms >= rangeStart && ms <= rangeEnd;
      const date = new Date(ms);

      if (inRange && monthAnchor === null) monthAnchor = date;

      week.push({
        key: String(ms),
        inRange,
        created,
        updated,
        total,
        level: inRange ? activityLevel(total, max) : 0,
        title: inRange
          ? `${dayFormatter.format(date)} · ${t("studio.dashboard.created")} ${created} / ${t("studio.dashboard.updated")} ${updated}`
          : "",
      });
    }
    if (monthAnchor !== null) {
      const month = monthAnchor.getMonth();
      if (month !== lastMonth) {
        monthLabels.push({
          label: monthFormatter.format(monthAnchor),
          weekIndex,
        });
        lastMonth = month;
      }
    }
    weeks.push(week);
    cursor += 7 * MS_DAY;
    weekIndex += 1;
  }

  return { weeks, monthLabels, activityTotal, max };
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

      <!-- 开发热力 = 近 30 天活动 + 项目状态构成 -->
      <article class="chart-card heat-card">
        <div class="heat-head">
          <h2>{{ $t("studio.dashboard.contributionsTitle") }}</h2>
          <p class="heat-count">
            {{
              $t("studio.dashboard.contributionsCount", {
                n: contributions.activityTotal,
              })
            }}
          </p>
        </div>
        <p class="sub">{{ $t("studio.dashboard.contributionsSub") }}</p>

        <div
          class="heat"
          role="img"
          :aria-label="
            $t('studio.dashboard.contributionsCount', {
              n: contributions.activityTotal,
            })
          "
        >
          <div class="heat-body">
            <div class="heat-weekdays" aria-hidden="true">
              <span class="heat-weekday-spacer"></span>
              <span>{{ $t("studio.dashboard.weekdayMon") }}</span>
              <span></span>
              <span>{{ $t("studio.dashboard.weekdayWed") }}</span>
              <span></span>
              <span>{{ $t("studio.dashboard.weekdayFri") }}</span>
              <span></span>
              <span></span>
            </div>
            <div class="heat-main">
              <div
                class="heat-months"
                aria-hidden="true"
                :style="{
                  gridTemplateColumns: `repeat(${contributions.weeks.length}, 12px)`,
                }"
              >
                <span
                  v-for="month in contributions.monthLabels"
                  :key="`${month.label}-${month.weekIndex}`"
                  class="heat-month"
                  :style="{ gridColumn: month.weekIndex + 1 }"
                >
                  {{ month.label }}
                </span>
              </div>
              <div
                class="heat-grid"
                :style="{
                  gridTemplateColumns: `repeat(${contributions.weeks.length}, 12px)`,
                }"
              >
                <div
                  v-for="(week, weekIndex) in contributions.weeks"
                  :key="weekIndex"
                  class="heat-week"
                >
                  <span
                    v-for="cell in week"
                    :key="cell.key"
                    class="heat-cell"
                    :class="[`level-${cell.level}`, { out: !cell.inRange }]"
                    :title="cell.title"
                  ></span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="heat-foot">
          <p class="status-legend">
            <span
              v-for="entry in stats?.statusCounts ?? []"
              :key="entry.status"
              class="status-item"
            >
              <b>{{ entry.count }}</b> {{ statusLabel(entry.status) }}
            </span>
          </p>
          <p class="heat-scale" aria-hidden="true">
            <span>{{ $t("studio.dashboard.less") }}</span>
            <span class="heat-cell level-0"></span>
            <span class="heat-cell level-1"></span>
            <span class="heat-cell level-2"></span>
            <span class="heat-cell level-3"></span>
            <span class="heat-cell level-4"></span>
            <span>{{ $t("studio.dashboard.more") }}</span>
          </p>
        </div>
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
/* GitHub contributions 热力 */
.heat-card {
  gap: 8px;
}
.heat-head {
  align-items: baseline;
  display: flex;
  flex-wrap: wrap;
  gap: 8px 12px;
  justify-content: space-between;
}
.heat-count {
  color: var(--muted-foreground);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  margin: 0;
}
.heat {
  overflow-x: auto;
  padding: 8px 0 4px;
}
.heat-body {
  align-items: start;
  display: grid;
  gap: 6px;
  grid-template-columns: 22px max-content;
}
.heat-weekdays {
  color: var(--subtle-foreground);
  display: grid;
  font-size: 9.5px;
  gap: 3px;
  grid-template-rows: 14px repeat(7, 12px);
  letter-spacing: 0.02em;
}
.heat-weekdays span {
  align-items: center;
  display: flex;
  height: 12px;
  line-height: 1;
}
.heat-weekday-spacer {
  height: 14px;
}
.heat-main {
  display: grid;
  gap: 3px;
}
.heat-months {
  display: grid;
  gap: 3px;
  min-height: 14px;
}
.heat-month {
  color: var(--subtle-foreground);
  font-size: 10px;
  letter-spacing: 0.04em;
  white-space: nowrap;
}
.heat-grid {
  display: grid;
  gap: 3px;
}
.heat-week {
  display: grid;
  gap: 3px;
  grid-template-rows: repeat(7, 12px);
}
.heat-cell {
  aspect-ratio: 1;
  background: color-mix(in srgb, var(--muted-foreground) 12%, transparent);
  border-radius: 2px;
  display: block;
  height: 12px;
  width: 12px;
}
.heat-cell.out {
  opacity: 0.35;
}
.heat-cell.level-1 {
  background: color-mix(in srgb, var(--primary) 28%, transparent);
}
.heat-cell.level-2 {
  background: color-mix(in srgb, var(--primary) 48%, transparent);
}
.heat-cell.level-3 {
  background: color-mix(in srgb, var(--primary) 72%, transparent);
}
.heat-cell.level-4 {
  background: var(--primary);
}
.heat-foot {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 10px 16px;
  justify-content: space-between;
  padding-top: 2px;
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
.heat-scale {
  align-items: center;
  color: var(--subtle-foreground);
  display: flex;
  font-size: 10px;
  gap: 3px;
  margin: 0;
}
.heat-scale .heat-cell {
  flex: none;
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
