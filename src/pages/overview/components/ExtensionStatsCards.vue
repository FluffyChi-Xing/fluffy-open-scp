<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import type {
  ExtensionStat,
  PackageStat,
  PackageStatistics,
} from "@/api/tauri";

interface Props {
  stats: PackageStatistics | null;
  loading: boolean;
  error: string;
}

const props = defineProps<Props>();
const expanded = ref(false);

/** lieflat 语义：无序类目多，色相按类目分配；unknown 固定用主题灰。 */
const palette = [
  "#4f46e5", "#0ea5e9", "#22a06b", "#f59e0b", "#e11d48",
  "#8b5cf6", "#14b8a6", "#ec4899", "#84cc16", "#f97316",
  "#06b6d4", "#a855f7", "#eab308", "#10b981", "#f43f5e",
  "#3b82f6", "#d946ef", "#65a30d", "#fb923c", "#2dd4bf",
];
const UNKNOWN_COLOR = "#8a8f9d";

interface ExtSlice {
  ext: string;
  count: number;
  size: number;
  known: boolean;
}

function mergeExtensions(extensions: ExtensionStat[]): ExtSlice[] {
  const merged: ExtSlice[] = extensions
    .filter((item) => item.known)
    .map((item) => ({
      ext: item.ext,
      count: item.count,
      size: item.decompressedSize,
      known: true,
    }));
  const unknown = extensions.filter((item) => !item.known);
  if (unknown.length) {
    merged.push({
      ext: "unknown",
      count: unknown.reduce((total, item) => total + item.count, 0),
      size: unknown.reduce((total, item) => total + item.decompressedSize, 0),
      known: false,
    });
  }
  return merged;
}

const slices = computed<ExtSlice[]>(() =>
  props.stats ? mergeExtensions(props.stats.extensions) : [],
);
const countSlices = computed(() =>
  [...slices.value].sort((a, b) => b.count - a.count),
);
const sizeSlices = computed(() =>
  [...slices.value].sort((a, b) => b.size - a.size),
);

function colorOf(item: ExtSlice, index: number): string {
  return item.known ? palette[index % palette.length] : UNKNOWN_COLOR;
}

/* ── F4 tick donut：环段按占比分角，段间留缝，中心放总数 ── */
interface DonutSegment {
  key: string;
  ext: string;
  count: number;
  size: number;
  known: boolean;
  color: string;
  path: string;
  percent: number;
}

function buildDonut(items: ExtSlice[]): DonutSegment[] {
  const total = items.reduce((sum, item) => sum + item.count, 0) || 1;
  const cx = 100;
  const cy = 100;
  const r0 = 62;
  const r1 = 86;
  const pad = 1.4; // 段间呼吸缝（度）
  let angle = -90;
  return items.map((item, index) => {
    const sweep = (item.count / total) * 360;
    const a0 = angle + pad / 2;
    const a1 = angle + sweep - pad / 2;
    angle += sweep;
    return {
      key: item.ext,
      ext: item.ext,
      count: item.count,
      size: item.size,
      known: item.known,
      color: colorOf(item, index),
      path: annulusPath(cx, cy, r0, r1, a0, Math.max(a1, a0 + 0.2)),
      percent: (item.count / total) * 100,
    };
  });
}

function polar(cx: number, cy: number, r: number, deg: number): [number, number] {
  const rad = (deg * Math.PI) / 180;
  return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)];
}

function annulusPath(
  cx: number,
  cy: number,
  r0: number,
  r1: number,
  a0: number,
  a1: number,
): string {
  const [x0o, y0o] = polar(cx, cy, r1, a0);
  const [x1o, y1o] = polar(cx, cy, r1, a1);
  const [x1i, y1i] = polar(cx, cy, r0, a1);
  const [x0i, y0i] = polar(cx, cy, r0, a0);
  const large = a1 - a0 > 180 ? 1 : 0;
  return [
    `M ${x0o} ${y0o}`,
    `A ${r1} ${r1} 0 ${large} 1 ${x1o} ${y1o}`,
    `L ${x1i} ${y1i}`,
    `A ${r0} ${r0} 0 ${large} 0 ${x0i} ${y0i}`,
    "Z",
  ].join(" ");
}

const donut = computed(() => buildDonut(countSlices.value));
const totalCount = computed(() =>
  countSlices.value.reduce((sum, item) => sum + item.count, 0),
);

/* ── F5 tick rows：横向行，长度 = 占总容量的真实比例 ── */
interface SizeRow {
  ext: string;
  size: number;
  known: boolean;
  color: string;
  percent: number;
}

const sizeTotal = computed(() =>
  sizeSlices.value.reduce((sum, item) => sum + item.size, 0) || 1,
);
const sizeRows = computed<SizeRow[]>(() =>
  sizeSlices.value.map((item, index) => ({
    ext: item.ext,
    size: item.size,
    known: item.known,
    color: colorOf(item, index),
    percent: (item.size / sizeTotal.value) * 100,
  })),
);

const coverage = computed(() => {
  const totals = props.stats?.totals;
  if (!totals || totals.decompressedSize === 0) return null;
  return (totals.knownDecompressed / totals.decompressedSize) * 100;
});

/* ── per-package 明细（紧凑）：小 donut + top 行条 ── */
interface PackageDetail {
  pkg: PackageStat;
  donut: DonutSegment[];
  rows: SizeRow[];
  knownPercent: number;
}

function buildPackageDetail(pkg: PackageStat): PackageDetail {
  const merged = mergeExtensions(pkg.extensions);
  const byCount = [...merged].sort((a, b) => b.count - a.count);
  const bySize = [...merged].sort((a, b) => b.size - a.size);
  const sizeTotalPkg = bySize.reduce((sum, item) => sum + item.size, 0) || 1;
  return {
    pkg,
    donut: buildDonut(byCount),
    rows: bySize.slice(0, 8).map((item, index) => ({
      ext: item.ext,
      size: item.size,
      known: item.known,
      color: colorOf(item, index),
      percent: (item.size / sizeTotalPkg) * 100,
    })),
    knownPercent:
      (pkg.knownDecompressed / (pkg.knownDecompressed + pkg.unknownDecompressed || 1)) *
      100,
  };
}

const packageDetails = computed(() =>
  (props.stats?.packages ?? []).map(buildPackageDetail),
);

/* ── 自绘 tooltip（吃主题 CSS 变量，明暗模式自动适配） ── */
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
  const offset = 14;
  tooltip.x = Math.min(event.clientX + offset, window.innerWidth - 240);
  tooltip.y = Math.min(event.clientY + offset, window.innerHeight - 120);
}
function hideTooltip() {
  tooltip.visible = false;
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = value;
  let unit = -1;
  do {
    size /= 1024;
    unit += 1;
  } while (size >= 1024 && unit < units.length - 1);
  return `${size.toFixed(size >= 100 ? 0 : 1)} ${units[unit]}`;
}
</script>

<template>
  <section class="stats-cards" :aria-label="$t('home.extensionCountCard')">
    <p v-if="error" class="stats-error" role="alert">
      {{ $t("home.statsLoadError") }}: {{ error }}
    </p>
    <template v-if="stats">
      <div class="stats-cards-grid">
        <article class="stats-card">
          <header class="stats-header">
            <div>
              <FTypography :header="3" spacing="none">{{
                $t("home.extensionCountCard")
              }}</FTypography>
              <FTypography paragraphy type="secondary">{{
                $t("home.extensionCountCardDescription")
              }}</FTypography>
            </div>
            <span class="coverage-chip">
              {{ $t("home.fileCount") }}:
              {{ totalCount.toLocaleString() }}
            </span>
          </header>
          <div class="donut-layout">
            <svg
              viewBox="0 0 200 200"
              class="donut"
              role="img"
              :aria-label="$t('home.extensionCountCard')"
            >
              <path
                v-for="seg in donut"
                :key="seg.key"
                :d="seg.path"
                :fill="seg.color"
                class="donut-seg"
                @mouseenter="
                  showTooltip($event, seg.ext, [
                    { label: $t('home.fileCount'), value: seg.count.toLocaleString() },
                    { label: '%', value: `${seg.percent.toFixed(2)}%` },
                    { label: $t('home.totalSize'), value: formatBytes(seg.size) },
                  ])
                "
                @mousemove="moveTooltip"
                @mouseleave="hideTooltip"
              />
              <text x="100" y="96" class="donut-center-value">
                {{ totalCount.toLocaleString() }}
              </text>
              <text x="100" y="114" class="donut-center-label">
                {{ $t("home.fileCount") }}
              </text>
            </svg>
            <ul class="legend">
              <li
                v-for="(item, index) in countSlices"
                :key="item.ext"
                @mouseenter="
                  showTooltip($event, item.ext, [
                    { label: $t('home.fileCount'), value: item.count.toLocaleString() },
                    {
                      label: '%',
                      value: `${((item.count / (totalCount || 1)) * 100).toFixed(2)}%`,
                    },
                    { label: $t('home.totalSize'), value: formatBytes(item.size) },
                  ])
                "
                @mousemove="moveTooltip"
                @mouseleave="hideTooltip"
              >
                <span class="legend-dot" :style="{ background: colorOf(item, index) }" />
                <span class="legend-name">{{ item.ext }}</span>
                <span class="legend-value">
                  {{ ((item.count / (totalCount || 1)) * 100).toFixed(1) }}%
                </span>
              </li>
            </ul>
          </div>
        </article>
        <article class="stats-card">
          <header class="stats-header">
            <div>
              <FTypography :header="3" spacing="none">{{
                $t("home.extensionSizeCard")
              }}</FTypography>
              <FTypography paragraphy type="secondary">{{
                $t("home.extensionSizeCardDescription")
              }}</FTypography>
            </div>
            <span
              v-if="coverage !== null"
              class="coverage-chip"
              :class="{ low: coverage < 90 }"
            >
              {{ $t("home.knownCoverage") }}: {{ coverage.toFixed(2) }}%
            </span>
          </header>
          <ul class="size-rows">
            <li
              v-for="row in sizeRows"
              :key="row.ext"
              class="size-row"
              @mouseenter="
                showTooltip($event, row.ext, [
                  { label: $t('home.totalSize'), value: formatBytes(row.size) },
                  { label: '%', value: `${row.percent.toFixed(2)}%` },
                ])
              "
              @mousemove="moveTooltip"
              @mouseleave="hideTooltip"
            >
              <span class="size-label" :class="{ unknown: !row.known }">{{
                row.ext
              }}</span>
              <span class="size-track">
                <span
                  class="size-fill"
                  :style="{
                    width: `${Math.max(row.percent, 0.25)}%`,
                    background: row.color,
                  }"
                />
              </span>
              <span class="size-percent">{{ row.percent.toFixed(1) }}%</span>
            </li>
          </ul>
        </article>
      </div>
      <button class="breakdown-toggle" type="button" @click="expanded = !expanded">
        <FIcon :name="expanded ? 'ChevronUp' : 'ChevronDown'" :size="16" aria-label="" />
        {{ expanded ? $t("home.hideBreakdown") : $t("home.showBreakdown") }}
      </button>
      <section v-if="expanded" class="package-breakdown">
        <FTypography :header="4" spacing="none">{{
          $t("home.packageBreakdown")
        }}</FTypography>
        <article
          v-for="detail in packageDetails"
          :key="detail.pkg.path"
          class="package-card"
        >
          <header class="package-head">
            <strong :title="detail.pkg.path">{{ detail.pkg.name }}</strong>
            <span class="muted">
              {{ detail.pkg.entryCount.toLocaleString() }} ·
              {{ formatBytes(detail.pkg.knownDecompressed + detail.pkg.unknownDecompressed) }}
            </span>
            <span class="coverage-chip small" :class="{ low: detail.knownPercent < 90 }">
              {{ $t("home.known") }} {{ detail.knownPercent.toFixed(1) }}%
            </span>
          </header>
          <div class="package-body">
            <svg viewBox="0 0 200 200" class="mini-donut" role="img">
              <path
                v-for="seg in detail.donut"
                :key="seg.key"
                :d="seg.path"
                :fill="seg.color"
                class="donut-seg"
                @mouseenter="
                  showTooltip($event, `${detail.pkg.name} · ${seg.ext}`, [
                    { label: $t('home.fileCount'), value: seg.count.toLocaleString() },
                    { label: '%', value: `${seg.percent.toFixed(2)}%` },
                    { label: $t('home.totalSize'), value: formatBytes(seg.size) },
                  ])
                "
                @mousemove="moveTooltip"
                @mouseleave="hideTooltip"
              />
              <text x="100" y="103" class="donut-center-value small">
                {{ detail.knownPercent.toFixed(0) }}%
              </text>
            </svg>
            <ul class="size-rows compact">
              <li
                v-for="row in detail.rows"
                :key="row.ext"
                class="size-row"
                @mouseenter="
                  showTooltip($event, `${detail.pkg.name} · ${row.ext}`, [
                    { label: $t('home.totalSize'), value: formatBytes(row.size) },
                    { label: '%', value: `${row.percent.toFixed(2)}%` },
                  ])
                "
                @mousemove="moveTooltip"
                @mouseleave="hideTooltip"
              >
                <span class="size-label" :class="{ unknown: !row.known }">{{
                  row.ext
                }}</span>
                <span class="size-track">
                  <span
                    class="size-fill"
                    :style="{
                      width: `${Math.max(row.percent, 0.25)}%`,
                      background: row.color,
                    }"
                  />
                </span>
                <span class="size-percent">{{ row.percent.toFixed(1) }}%</span>
              </li>
            </ul>
          </div>
        </article>
      </section>
    </template>
    <div v-else-if="loading" class="stats-empty" role="status">
      <FIcon name="LoaderCircle" :size="20" aria-label="" />{{
        $t("common.loading")
      }}
    </div>
    <div v-else class="stats-empty">
      <FIcon name="PieChart" :size="24" aria-label="" />
      <FTypography paragraphy type="secondary">{{
        $t("home.statsEmpty")
      }}</FTypography>
    </div>
    <Teleport to="body">
      <div
        v-if="tooltip.visible"
        class="stats-tooltip"
        :style="{ left: `${tooltip.x}px`, top: `${tooltip.y}px` }"
        aria-hidden="true"
      >
        <strong class="stats-tooltip-title">{{ tooltip.title }}</strong>
        <span v-for="row in tooltip.rows" :key="row.label" class="stats-tooltip-row">
          <span class="muted">{{ row.label }}</span>
          <strong>{{ row.value }}</strong>
        </span>
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
.stats-cards {
  display: grid;
  gap: 14px;
}
.stats-cards-grid {
  align-items: start; /* 卡片高度由各自内容决定，互不拉伸 */
  display: grid;
  gap: 14px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
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
  margin-bottom: 10px;
}
.coverage-chip {
  background: var(--accent);
  border-radius: var(--radius-sm);
  color: var(--primary);
  font-size: 11px;
  font-weight: 700;
  padding: 4px 8px;
  white-space: nowrap;
}
.coverage-chip.small {
  font-size: 10px;
  padding: 2px 7px;
}
.coverage-chip.low {
  background: color-mix(in oklab, var(--warning) 18%, transparent);
  color: var(--warning);
}

/* ── donut + 图例 ── */
.donut-layout {
  align-items: center;
  display: grid;
  gap: 16px;
  grid-template-columns: minmax(170px, 210px) minmax(0, 1fr);
}
.donut {
  display: block;
  width: 100%;
}
.donut-seg {
  cursor: default;
  stroke: var(--surface);
  stroke-width: 1;
  transition: opacity 120ms ease;
}
.donut-seg:hover {
  opacity: 0.75;
}
.donut-center-value {
  fill: var(--foreground);
  dominant-baseline: middle;
  font-size: 24px;
  font-weight: 800;
  text-anchor: middle;
}
.donut-center-value.small {
  font-size: 26px;
}
.donut-center-label {
  fill: var(--muted-foreground);
  dominant-baseline: middle;
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-anchor: middle;
  text-transform: uppercase;
}
.legend {
  display: grid;
  gap: 2px 14px;
  grid-auto-flow: column;
  grid-template-rows: repeat(10, auto);
  list-style: none;
  margin: 0;
  max-height: 220px;
  overflow-y: auto;
  padding: 0;
}
.legend li {
  align-items: center;
  border-radius: var(--radius-sm);
  display: flex;
  font-size: 11px;
  gap: 7px;
  padding: 2px 5px;
}
.legend li:hover {
  background: var(--accent);
}
.legend-dot {
  border-radius: 2px;
  flex: none;
  height: 8px;
  width: 8px;
}
.legend-name {
  color: var(--foreground);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.legend-value {
  color: var(--muted-foreground);
  margin-inline-start: auto;
  font-variant-numeric: tabular-nums;
}

/* ── size tick rows ── */
.size-rows {
  display: grid;
  gap: 5px;
  list-style: none;
  margin: 0;
  max-height: 340px;
  overflow-y: auto;
  padding: 0;
}
.size-row {
  align-items: center;
  display: grid;
  gap: 10px;
  grid-template-columns: 84px minmax(0, 1fr) 48px;
}
.size-label {
  color: var(--foreground);
  font-size: 11px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.size-label.unknown {
  color: var(--muted-foreground);
  font-style: italic;
}
.size-track {
  background: color-mix(in oklab, var(--muted-foreground) 12%, transparent);
  border-radius: 999px;
  display: block;
  height: 12px;
  overflow: hidden;
}
.size-fill {
  border-radius: 999px;
  display: block;
  height: 100%;
  min-width: 2px;
  transition: width 400ms cubic-bezier(0.16, 1, 0.3, 1);
}
.size-percent {
  color: var(--muted-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  text-align: right;
}

/* ── 展开：按 package 明细（紧凑） ── */
.breakdown-toggle {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 12px;
  font-weight: 700;
  gap: 7px;
  justify-self: start;
  min-height: 34px;
  padding: 0 13px;
  transition: color 140ms ease;
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
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  min-width: 0;
  padding: 12px 16px;
}
.package-head {
  align-items: center;
  display: flex;
  font-size: 12px;
  gap: 10px;
  margin-bottom: 8px;
}
.package-head strong {
  font-size: 13px;
}
.package-head .muted {
  color: var(--muted-foreground);
  font-size: 11px;
  margin-inline-start: auto;
}
.package-body {
  align-items: center;
  display: grid;
  gap: 18px;
  grid-template-columns: 128px minmax(0, 1fr);
}
.mini-donut {
  display: block;
  width: 128px;
}
.size-rows.compact {
  gap: 3px;
  max-height: none;
}
.size-rows.compact .size-row {
  grid-template-columns: 84px minmax(0, 1fr) 48px;
}
.size-rows.compact .size-track {
  height: 9px;
}

.stats-empty {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
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
@media (max-width: 850px) {
  .stats-cards-grid,
  .donut-layout,
  .package-body {
    grid-template-columns: 1fr;
  }
  .legend {
    grid-auto-flow: row;
    grid-template-rows: none;
  }
}
</style>

<style>
/* tooltip 经 Teleport 挂到 body，需非 scoped 才能命中；全部走主题变量。 */
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
  justify-content: space-between;
  gap: 20px;
}
.stats-tooltip-row .muted {
  color: var(--muted-foreground);
}
</style>
