<script setup lang="ts">
/**
 * 仪表盘：渲染各阶段耗时卡。
 *
 * 图型 = Lieflat Charts **F12 Dumbbell Queue**（哑铃队列）：
 * 每行一个阶段，空心点 = 平均、实心点 = p95，两点连成哑铃；行按平均降序。
 * 数值与位置严格成正比，轴从 0 起不断轴。
 *
 * 两个刻意的实现决定：
 * 1. **用 HTML/CSS 行而非 SVG**：卡片宽度跨度很大（概览页整卡约 1900px），
 *    SVG + viewBox 会等比放大到几百像素高，与页面严重不协调。改为固定行高
 *    （不随宽度缩放）+ 百分比定位，字号是真实 px，宽度自适应轨道自然撑满。
 * 2. **数值放右侧定宽列而非浮动标签**：平均与 p95 接近时（如 543/686）
 *    浮动标签必然重叠；定宽列对齐后既无重叠，也更像工具的读数栏。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import type { RenderStageStat, RenderTelemetrySummary } from "@/api/tauri";

const props = defineProps<{
  summary: RenderTelemetrySummary | null;
  loading: boolean;
  error: string;
}>();
const emit = defineEmits<{ clear: [] }>();

const { t } = useI18n();

const STAGE_LABELS: Record<string, string> = {
  model_load: "package.renderStageModelLoad",
  texture_compose: "package.renderStageTextureCompose",
  lot_render: "package.renderStageLotRender",
  decal_render: "package.renderStageDecalRender",
  scene_rebuild: "package.renderStageSceneRebuild",
};

function stageLabel(stage: string): string {
  const key = STAGE_LABELS[stage];
  return key ? t(key) : stage;
}

function formatMs(value: number): string {
  if (!Number.isFinite(value)) return "—";
  return value >= 100 ? `${value.toFixed(0)} ms` : `${value.toFixed(1)} ms`;
}

/** 轴刻度用紧凑写法（去单位后缀，单位在轴尾统一标一次）。 */
function formatTick(value: number): string {
  if (value >= 1000) return `${(value / 1000).toFixed(1)}s`;
  return `${Math.round(value)}`;
}

const stages = computed<RenderStageStat[]>(() =>
  [...(props.summary?.stages ?? [])].sort((a, b) => b.avgMs - a.avgMs),
);

/** 横轴定义域：0 → 最大 p95 留 6% 余量（不截断）。 */
const domainMax = computed(
  () => Math.max(1, ...stages.value.map((stage) => Math.max(stage.p95Ms, stage.avgMs))) * 1.06,
);

function percent(value: number): number {
  return Math.min(100, Math.max(0, (value / domainMax.value) * 100));
}

const ticks = computed(() =>
  [0, 0.5, 1].map((fraction) => ({
    percent: fraction * 100,
    label: formatTick(domainMax.value * fraction),
  })),
);

/** 滚入视野才播入场动画。 */
const root = ref<HTMLElement | null>(null);
const revealed = ref(false);
let observer: IntersectionObserver | null = null;

onMounted(() => {
  if (typeof IntersectionObserver === "undefined" || !root.value) {
    revealed.value = true;
    return;
  }
  observer = new IntersectionObserver((entries) => {
    if (entries.some((entry) => entry.isIntersecting)) {
      revealed.value = true;
      observer?.disconnect();
      observer = null;
    }
  });
  observer.observe(root.value);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
});

function confirmClear(): void {
  if (window.confirm(t("home.renderClearConfirm"))) emit("clear");
}
</script>

<template>
  <section
    ref="root"
    class="stats-card"
    :class="{ revealed }"
    :aria-label="$t('home.renderTimings')"
  >
    <header class="stats-header">
      <div>
        <FTypography :header="3" spacing="none">{{ $t("home.renderTimings") }}</FTypography>
        <p class="stats-sub">
          <span class="legend-dot legend-ink" aria-hidden="true" />{{ $t("home.renderLegendP95") }}
          <span class="sep">·</span>
          <span class="legend-dot legend-hollow" aria-hidden="true" />{{
            $t("home.renderLegendAvg")
          }}
          <template v-if="summary">
            <span class="sep">·</span>
            {{ $t("home.renderWindow", { days: summary.windowDays, count: summary.totalCount }) }}
          </template>
        </p>
      </div>
      <button
        v-if="summary && summary.totalCount > 0"
        type="button"
        class="clear-button"
        @click="confirmClear"
      >
        {{ $t("home.renderClear") }}
      </button>
    </header>

    <p v-if="error" class="state error" role="alert">{{ error }}</p>
    <div v-else-if="loading && !summary" class="state" role="status">
      <FIcon name="LoaderCircle" :size="18" aria-label="" />{{ $t("common.loading") }}
    </div>
    <div v-else-if="!stages.length" class="state">
      <FIcon name="Activity" :size="22" aria-label="" />
      <FTypography paragraphy type="secondary">{{ $t("home.renderEmpty") }}</FTypography>
    </div>
    <div v-else class="chart" role="img" :aria-label="$t('home.renderTimings')">
      <div
        v-for="(stage, index) in stages"
        :key="stage.stage"
        class="stage-row anim"
        :style="{ animationDelay: `${index * 0.06}s` }"
      >
        <span class="stage-name">{{ stageLabel(stage.stage) }}</span>
        <span class="track">
          <span class="rail" aria-hidden="true" />
          <span
            class="connector"
            aria-hidden="true"
            :style="{ left: `${percent(stage.avgMs)}%`, width: `${percent(stage.p95Ms) - percent(stage.avgMs)}%` }"
          />
          <span class="dot dot-hollow" aria-hidden="true" :style="{ left: `${percent(stage.avgMs)}%` }" />
          <span class="dot dot-ink" aria-hidden="true" :style="{ left: `${percent(stage.p95Ms)}%` }" />
        </span>
        <span class="reading">
          <span class="reading-avg">{{ formatMs(stage.avgMs) }}</span>
          <span class="arrow" aria-hidden="true">→</span>
          <strong class="reading-p95">{{ formatMs(stage.p95Ms) }}</strong>
        </span>
        <span class="stage-count">{{ t("home.renderSamples", { count: stage.count }) }}</span>
      </div>

      <!-- 底部刻度：与轨道列对齐 -->
      <div class="axis anim" :style="{ animationDelay: `${stages.length * 0.06}s` }">
        <span />
        <span class="axis-track">
          <span
            v-for="tick in ticks"
            :key="tick.percent"
            class="axis-tick"
            :style="{ left: `${tick.percent}%` }"
          >
            <span class="axis-mark" aria-hidden="true" />
            <span class="axis-label">{{ tick.label }}</span>
          </span>
          <span class="axis-unit">ms</span>
        </span>
        <span />
        <span />
      </div>
    </div>
  </section>
</template>

<style scoped>
.stats-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  min-width: 0;
  padding: 16px 18px;
}
.stats-header {
  align-items: flex-start;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  margin-bottom: 12px;
}
.stats-sub {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex-wrap: wrap;
  font-size: 11px;
  gap: 4px;
  margin: 4px 0 0;
}
.sep {
  opacity: 0.5;
}
.legend-dot {
  border-radius: 50%;
  display: inline-block;
  height: 8px;
  width: 8px;
}
.legend-ink {
  background: var(--foreground);
}
.legend-hollow {
  background: var(--surface);
  border: 1.3px solid var(--foreground);
}
.clear-button {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  font-size: 11px;
  padding: 3px 8px;
}
.clear-button:hover {
  border-color: var(--muted-foreground);
  color: var(--foreground);
}
.state {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 14px 0;
  text-align: center;
}
.state.error {
  color: var(--danger, #d9534f);
}

/* ── 图表：固定行高，宽度自适应；不随卡片宽度缩放字号 ── */
.chart {
  --col-name: 132px;
  --col-reading: 138px;
  --col-count: 58px;
  display: flex;
  flex-direction: column;
}
.stage-row,
.axis {
  align-items: center;
  display: grid;
  gap: 14px;
  grid-template-columns: var(--col-name) minmax(80px, 1fr) var(--col-reading) var(--col-count);
}
.stage-row {
  height: 30px;
}
.stage-name {
  color: var(--muted-foreground);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.track {
  display: block;
  height: 12px;
  position: relative;
}
/* 发丝轨道（贯穿整行，作为读数基准） */
.rail {
  background: var(--border);
  height: 1px;
  left: 0;
  position: absolute;
  right: 0;
  top: 50%;
}
/* 平均 → p95 的波动区间 */
.connector {
  background: var(--muted-foreground);
  border-radius: 999px;
  height: 2px;
  position: absolute;
  top: calc(50% - 1px);
}
.dot {
  border-radius: 50%;
  height: 11px;
  position: absolute;
  top: calc(50% - 5.5px);
  transform: translateX(-50%);
  width: 11px;
}
.dot-hollow {
  background: var(--surface);
  border: 1.5px solid var(--foreground);
}
.dot-ink {
  background: var(--foreground);
}
.reading {
  align-items: baseline;
  display: flex;
  font-size: 11.5px;
  gap: 5px;
  justify-content: flex-end;
  white-space: nowrap;
}
.reading-avg {
  color: var(--muted-foreground);
  font-variant-numeric: tabular-nums;
}
.reading-p95 {
  color: var(--foreground);
  font-variant-numeric: tabular-nums;
}
.arrow {
  color: var(--muted-foreground);
  opacity: 0.6;
}
.stage-count {
  color: var(--muted-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  text-align: end;
}

/* ── 底部刻度 ── */
.axis {
  height: 26px;
  margin-top: 2px;
}
.axis-track {
  display: block;
  height: 100%;
  position: relative;
}
.axis-tick {
  position: absolute;
  top: 0;
  transform: translateX(-50%);
}
.axis-mark {
  background: var(--border);
  display: block;
  height: 4px;
  margin: 0 auto;
  width: 1px;
}
.axis-label {
  color: var(--muted-foreground);
  display: block;
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  margin-top: 3px;
}
.axis-unit {
  bottom: 0;
  color: var(--subtle-foreground, var(--muted-foreground));
  font-size: 10px;
  position: absolute;
  right: 0;
}

/* ── 入场：快进快停，不弹跳 ── */
.revealed .anim {
  animation-duration: 0.42s;
  animation-fill-mode: both;
  animation-name: rise-in;
  animation-timing-function: cubic-bezier(0.165, 0.84, 0.44, 1);
}
@keyframes rise-in {
  from {
    opacity: 0;
    transform: translateY(3px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
@media (prefers-reduced-motion: reduce) {
  .revealed .anim {
    animation: none;
  }
}
@media (max-width: 720px) {
  .chart {
    --col-name: 88px;
    --col-reading: 112px;
    --col-count: 48px;
  }
}
</style>
