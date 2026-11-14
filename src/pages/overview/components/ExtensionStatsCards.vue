<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import type { PackageStatistics } from "@/api/tauri";
import {
  SPARK_COLOR,
  compressionRatio,
  countRankBars,
  formatBytes,
  formatCompact,
  formatPercent,
  ghostSparkPath,
  knownCoveragePercent,
  mergeExtensions,
  packageSparks,
  sizeRankBars,
  sparklinePath,
  topSizeExtension,
} from "./extensionStatsModel";

interface Props {
  stats: PackageStatistics | null;
  loading: boolean;
  error: string;
}

const props = defineProps<Props>();
const expanded = ref(false);
const { t } = useI18n();

const slices = computed(() =>
  props.stats ? mergeExtensions(props.stats.extensions) : [],
);
const countBars = computed(() => countRankBars(slices.value, 8));
const sizeBars = computed(() => sizeRankBars(slices.value, 6));
const coverage = computed(() =>
  props.stats ? knownCoveragePercent(props.stats) : 0,
);
const ratio = computed(() => (props.stats ? compressionRatio(props.stats) : null));
const entrySpark = computed(() =>
  props.stats ? packageSparks(props.stats.packages, "entryCount") : [],
);
const sizeSpark = computed(() =>
  props.stats ? packageSparks(props.stats.packages, "decompressed") : [],
);
const topHeavy = computed(() => topSizeExtension(slices.value));
const totals = computed(() => props.stats?.totals ?? null);

const entryPath = computed(() => sparklinePath(entrySpark.value));
const sizePath = computed(() => sparklinePath(sizeSpark.value));
const ghostPath = ghostSparkPath();

interface TooltipState {
  visible: boolean;
  x: number;
  y: number;
  title: string;
  rows: { label: string; value: string }[];
}
const tooltip = reactive<TooltipState>({
  visible: false,
  x: 0,
  y: 0,
  title: "",
  rows: [],
});

function showTooltip(
  event: MouseEvent,
  title: string,
  rows: { label: string; value: string }[],
) {
  tooltip.title = title;
  tooltip.rows = rows;
  moveTooltip(event);
  tooltip.visible = true;
}
function moveTooltip(event: MouseEvent) {
  tooltip.x = Math.min(event.clientX + 14, window.innerWidth - 240);
  tooltip.y = Math.min(event.clientY + 14, window.innerHeight - 120);
}
function hideTooltip() {
  tooltip.visible = false;
}

function sparkTip(points: { label: string; value: number }[], format: (n: number) => string) {
  return (event: MouseEvent) => {
    if (!points.length) return;
    const rect = (event.currentTarget as Element).getBoundingClientRect();
    const ratio = Math.min(
      0.999,
      Math.max(0, (event.clientX - rect.left) / rect.width),
    );
    const index = Math.min(points.length - 1, Math.floor(ratio * points.length));
    const point = points[index]!;
    showTooltip(event, point.label, [
      { label: t("home.analyticsValue"), value: format(point.value) },
    ]);
  };
}
</script>

<template>
  <section class="analytics" :aria-label="t('home.analyticsSection')">
    <header class="analytics-toolbar">
      <div>
        <FTypography :header="3" spacing="none">{{ t("home.analyticsSection") }}</FTypography>
        <FTypography paragraphy type="secondary">{{
          t("home.analyticsSectionDescription")
        }}</FTypography>
      </div>
      <span class="scope-chip">{{ t("home.analyticsScope") }}</span>
    </header>

    <p v-if="error" class="stats-error" role="alert">
      {{ t("home.statsLoadError") }}: {{ error }}
    </p>

    <template v-if="stats && totals">
      <div class="analytics-grid">
        <!-- 宽卡：数量构成（与体积主导正交） -->
        <article class="metric-card span-8">
          <header class="metric-head">
            <span class="metric-title">{{ t("home.cardCountMix") }}</span>
          </header>
          <div class="metric-value-row">
            <strong class="metric-value">{{ formatCompact(totals.entryCount) }}</strong>
            <span class="metric-sub">{{ t("home.fileCount") }}</span>
          </div>
          <ul class="rank-bars" :aria-label="t('home.cardCountMix')">
            <li
              v-for="bar in countBars"
              :key="bar.key"
              @mouseenter="
                showTooltip($event, bar.label, [
                  { label: t('home.fileCount'), value: bar.value.toLocaleString() },
                  { label: '%', value: formatPercent(bar.percent, 1) },
                ])
              "
              @mousemove="moveTooltip"
              @mouseleave="hideTooltip"
            >
              <span class="rank-label">{{ bar.label }}</span>
              <span class="rank-track">
                <span
                  class="rank-fill"
                  :style="{ width: `${bar.percent}%`, color: bar.color }"
                />
              </span>
              <span class="rank-pct">{{ formatPercent(bar.percent, 1) }}</span>
            </li>
          </ul>
          <p class="metric-note">{{ t("home.cardCountMixHint") }}</p>
        </article>

        <!-- 窄卡：可识别覆盖 -->
        <article class="metric-card span-4">
          <header class="metric-head">
            <span class="metric-title">{{ t("home.cardCoverage") }}</span>
          </header>
          <div class="metric-value-row">
            <strong class="metric-value">{{ formatPercent(coverage, 2) }}</strong>
            <span
              class="trend"
              :class="coverage >= 90 ? 'up' : coverage >= 70 ? 'flat' : 'down'"
            >
              {{ t("home.known") }}
            </span>
          </div>
          <div class="coverage-visual" aria-hidden="true">
            <div class="coverage-track">
              <div class="coverage-fill" :style="{ width: `${Math.min(100, coverage)}%` }" />
            </div>
            <div class="coverage-split">
              <span>{{ formatBytes(totals.knownDecompressed) }}</span>
              <span class="muted">{{ formatBytes(totals.unknownDecompressed) }}</span>
            </div>
          </div>
          <p class="metric-note">{{ t("home.cardCoverageHint") }}</p>
        </article>

        <!-- 资源条目 -->
        <article class="metric-card span-3">
          <header class="metric-head">
            <span class="metric-title">{{ t("home.cardEntries") }}</span>
          </header>
          <div class="metric-value-row">
            <strong class="metric-value">{{ formatCompact(totals.entryCount) }}</strong>
            <span class="metric-sub">{{ t("home.acrossPackages", { n: stats.packages.length }) }}</span>
          </div>
          <svg
            class="spark"
            viewBox="0 0 100 36"
            preserveAspectRatio="none"
            role="img"
            :aria-label="t('home.cardEntries')"
            @mousemove="sparkTip(entrySpark, (n) => n.toLocaleString())($event)"
            @mouseleave="hideTooltip"
          >
            <path
              v-if="entryPath"
              :d="entryPath"
              fill="none"
              :stroke="SPARK_COLOR"
              class="spark-line"
            />
            <template v-else>
              <path :d="ghostPath" fill="none" class="spark-ghost" />
              <text x="50" y="20" class="spark-empty">{{ t("home.noSparkData") }}</text>
            </template>
          </svg>
        </article>

        <!-- 解压容量 -->
        <article class="metric-card span-3">
          <header class="metric-head">
            <span class="metric-title">{{ t("home.cardPayload") }}</span>
          </header>
          <div class="metric-value-row">
            <strong class="metric-value">{{ formatBytes(totals.decompressedSize) }}</strong>
            <span class="metric-sub">{{ t("home.decompressed") }}</span>
          </div>
          <svg
            class="spark"
            viewBox="0 0 100 36"
            preserveAspectRatio="none"
            role="img"
            :aria-label="t('home.cardPayload')"
            @mousemove="sparkTip(sizeSpark, formatBytes)($event)"
            @mouseleave="hideTooltip"
          >
            <path
              v-if="sizePath"
              :d="sizePath"
              fill="none"
              :stroke="SPARK_COLOR"
              class="spark-line"
            />
            <template v-else>
              <path :d="ghostPath" fill="none" class="spark-ghost" />
              <text x="50" y="20" class="spark-empty">{{ t("home.noSparkData") }}</text>
            </template>
          </svg>
        </article>

        <!-- 压缩效率 -->
        <article class="metric-card span-3">
          <header class="metric-head">
            <span class="metric-title">{{ t("home.cardCompression") }}</span>
          </header>
          <div class="metric-value-row">
            <strong class="metric-value">
              {{ ratio == null ? "—" : `${ratio.toFixed(2)}×` }}
            </strong>
            <span class="metric-sub">{{ t("home.cardCompressionSub") }}</span>
          </div>
          <div class="dual-bars" aria-hidden="true">
            <div class="dual-row">
              <span>{{ t("home.stored") }}</span>
              <span class="dual-track">
                <span
                  class="dual-fill stored"
                  :style="{
                    width: `${
                      totals.decompressedSize
                        ? Math.min(
                            100,
                            ((totals.storedSize || totals.fileSize) / totals.decompressedSize) *
                              100,
                          )
                        : 0
                    }%`,
                  }"
                />
              </span>
            </div>
            <div class="dual-row">
              <span>{{ t("home.decompressed") }}</span>
              <span class="dual-track">
                <span class="dual-fill full" />
              </span>
            </div>
          </div>
          <p class="metric-note">{{ t("home.cardCompressionHint") }}</p>
        </article>

        <!-- 体积主导类型（≠数量构成） -->
        <article class="metric-card span-3">
          <header class="metric-head">
            <span class="metric-title">{{ t("home.cardHeavyType") }}</span>
          </header>
          <div class="metric-value-row">
            <strong class="metric-value">{{ topHeavy?.ext ?? "—" }}</strong>
            <span class="metric-sub">
              {{ topHeavy ? formatBytes(topHeavy.size) : t("home.noSparkData") }}
            </span>
          </div>
          <ul class="mini-ranks">
            <li v-for="bar in sizeBars" :key="bar.key">
              <span
                class="mini-swatch"
                :style="{ background: bar.color }"
              />
              <span class="mini-label">{{ bar.label }}</span>
              <span class="mini-pct">{{ formatPercent(bar.percent, 1) }}</span>
            </li>
          </ul>
        </article>
      </div>

      <button class="breakdown-toggle" type="button" @click="expanded = !expanded">
        <FIcon
          :name="expanded ? 'ChevronUp' : 'ChevronDown'"
          :size="16"
          aria-label=""
        />
        {{ expanded ? t("home.hideBreakdown") : t("home.showBreakdown") }}
      </button>

      <section v-if="expanded" class="package-breakdown">
        <FTypography :header="4" spacing="none">{{
          t("home.packageBreakdown")
        }}</FTypography>
        <article
          v-for="pkg in stats.packages"
          :key="pkg.path"
          class="package-card"
        >
          <header class="package-head">
            <strong :title="pkg.path">{{ pkg.name }}</strong>
            <span class="muted">
              {{ pkg.entryCount.toLocaleString() }} ·
              {{ formatBytes(pkg.knownDecompressed + pkg.unknownDecompressed) }}
            </span>
          </header>
          <ul class="mini-ranks">
            <li
              v-for="bar in sizeRankBars(mergeExtensions(pkg.extensions), 5)"
              :key="`${pkg.path}:${bar.key}`"
            >
              <span class="mini-swatch" :style="{ background: bar.color }" />
              <span class="mini-label">{{ bar.label }}</span>
              <span class="mini-pct">{{ formatPercent(bar.percent, 1) }}</span>
            </li>
          </ul>
        </article>
      </section>
    </template>

    <div v-else-if="loading" class="stats-empty" role="status">
      <FIcon name="LoaderCircle" :size="20" aria-label="" />
      {{ t("common.loading") }}
    </div>
    <div v-else class="stats-empty">
      <FIcon name="PieChart" :size="24" aria-label="" />
      <FTypography paragraphy type="secondary">{{ t("home.statsEmpty") }}</FTypography>
    </div>

    <Teleport to="body">
      <div
        v-if="tooltip.visible"
        class="stats-tooltip"
        :style="{ left: `${tooltip.x}px`, top: `${tooltip.y}px` }"
        aria-hidden="true"
      >
        <strong class="stats-tooltip-title">{{ tooltip.title }}</strong>
        <span
          v-for="row in tooltip.rows"
          :key="row.label"
          class="stats-tooltip-row"
        >
          <span class="muted">{{ row.label }}</span>
          <strong>{{ row.value }}</strong>
        </span>
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
.analytics {
  display: grid;
  gap: 12px;
}
.analytics-toolbar {
  align-items: flex-start;
  display: flex;
  gap: 12px;
  justify-content: space-between;
}
.scope-chip {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--muted-foreground);
  font-size: 12px;
  font-weight: 600;
  padding: 6px 12px;
  white-space: nowrap;
}
.analytics-grid {
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(12, minmax(0, 1fr));
}
.metric-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
  padding: 16px 18px 14px;
}
.span-8 { grid-column: span 8; }
.span-4 { grid-column: span 4; }
.span-3 { grid-column: span 3; }
.metric-head {
  align-items: center;
  display: flex;
  justify-content: space-between;
}
.metric-title {
  color: var(--muted-foreground);
  font-size: 12px;
  font-weight: 600;
}
.metric-value-row {
  align-items: baseline;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.metric-value {
  color: var(--foreground);
  font-size: 28px;
  font-weight: 800;
  letter-spacing: -0.03em;
  line-height: 1.1;
}
.metric-sub {
  color: var(--muted-foreground);
  font-size: 12px;
}
.trend {
  border-radius: 999px;
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
}
.trend.up {
  background: color-mix(in srgb, var(--success) 16%, transparent);
  color: var(--success);
}
.trend.flat {
  background: color-mix(in srgb, var(--warning) 16%, transparent);
  color: var(--warning);
}
.trend.down {
  background: color-mix(in srgb, var(--danger) 14%, transparent);
  color: var(--danger);
}
.metric-note {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.45;
  margin: auto 0 0;
}
.rank-bars {
  display: grid;
  gap: 6px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.rank-bars li {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: 72px minmax(0, 1fr) 44px;
}
.rank-label {
  color: var(--foreground);
  font-size: 11px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rank-track {
  background: color-mix(in oklab, var(--muted-foreground) 12%, transparent);
  height: 12px;
  overflow: hidden;
}
.rank-fill {
  background: repeating-linear-gradient(
    90deg,
    currentColor 0 2.5px,
    transparent 2.5px 5px
  );
  display: block;
  height: 100%;
  opacity: 0.92;
}
.rank-pct {
  color: var(--muted-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  text-align: end;
}
.coverage-visual {
  display: grid;
  gap: 8px;
  margin-top: 4px;
}
.coverage-track {
  background: color-mix(in oklab, var(--muted-foreground) 14%, transparent);
  border-radius: 999px;
  height: 10px;
  overflow: hidden;
}
.coverage-fill {
  background: var(--success);
  height: 100%;
  transition: width 400ms cubic-bezier(0.16, 1, 0.3, 1);
}
.coverage-split {
  color: var(--foreground);
  display: flex;
  font-size: 11px;
  font-weight: 600;
  justify-content: space-between;
}
.coverage-split .muted,
.muted {
  color: var(--muted-foreground);
}
.spark {
  display: block;
  height: 56px;
  margin-top: auto;
  width: 100%;
}
.spark-line {
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
}
.spark-ghost {
  opacity: 0.28;
  stroke: var(--muted-foreground);
  stroke-width: 1.4;
}
.spark-empty {
  fill: var(--muted-foreground);
  font-size: 7px;
  font-weight: 700;
  text-anchor: middle;
}
.dual-bars {
  display: grid;
  gap: 8px;
  margin-top: 2px;
}
.dual-row {
  align-items: center;
  color: var(--muted-foreground);
  display: grid;
  font-size: 11px;
  gap: 8px;
  grid-template-columns: 56px minmax(0, 1fr);
}
.dual-track {
  background: color-mix(in oklab, var(--muted-foreground) 12%, transparent);
  height: 8px;
  overflow: hidden;
}
.dual-fill {
  display: block;
  height: 100%;
}
.dual-fill.stored {
  background: var(--info, #1280FE);
}
.dual-fill.full {
  background: color-mix(in srgb, var(--foreground) 35%, transparent);
  width: 100%;
}
.mini-ranks {
  display: grid;
  gap: 4px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.mini-ranks li {
  align-items: center;
  display: grid;
  gap: 6px;
  grid-template-columns: 8px minmax(0, 1fr) 40px;
}
.mini-swatch {
  border-radius: 2px;
  height: 8px;
  width: 8px;
}
.mini-label {
  color: var(--foreground);
  font-size: 11px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mini-pct {
  color: var(--muted-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  text-align: end;
}
.breakdown-toggle {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 12px;
  font-weight: 700;
  gap: 7px;
  justify-self: start;
  min-height: 34px;
  padding: 0 13px;
}
.breakdown-toggle:hover {
  color: var(--foreground);
}
.package-breakdown {
  display: grid;
  gap: 10px;
}
.package-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 12px 14px;
}
.package-head {
  align-items: center;
  display: flex;
  gap: 10px;
  margin-bottom: 8px;
}
.package-head strong {
  font-size: 13px;
}
.package-head .muted {
  font-size: 11px;
  margin-inline-start: auto;
}
.stats-empty {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 16px;
  color: var(--muted-foreground);
  display: flex;
  gap: 8px;
  justify-content: center;
  min-height: 100px;
  padding: 20px;
}
.stats-error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
}
@media (max-width: 1100px) {
  .span-8,
  .span-4,
  .span-3 {
    grid-column: span 6;
  }
}
@media (max-width: 720px) {
  .analytics-grid {
    grid-template-columns: 1fr;
  }
  .span-8,
  .span-4,
  .span-3 {
    grid-column: span 1;
  }
  .analytics-toolbar {
    flex-direction: column;
  }
}
</style>

<style>
.stats-tooltip {
  background: var(--surface-elevated, var(--surface));
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  color: var(--foreground);
  display: grid;
  gap: 4px;
  min-width: 160px;
  padding: 8px 11px;
  pointer-events: none;
  position: fixed;
  z-index: 1000;
}
.stats-tooltip-title {
  font-size: 12px;
  font-weight: 700;
}
.stats-tooltip-row {
  align-items: center;
  display: flex;
  font-size: 12px;
  gap: 20px;
  justify-content: space-between;
}
.stats-tooltip-row .muted {
  color: var(--muted-foreground);
}
</style>
