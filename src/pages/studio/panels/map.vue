<script setup lang="ts">
/**
 * 地图开发面板：区域地形合成预览。
 * 左侧 = 可交互地图预览（滚轮缩放 / 中键或空格拖动 / 米制标尺 / HUD），
 * 右侧 = 区域属性与图层区（未来叠加资源多层视图与地图笔刷）。
 * 交互与标尺完全对齐 RasterCanvas 的实现模式。
 * 渲染管线：sc_properties::region_map（341-tile 金字塔 + 全局水位面 3336）。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import { useGamePackagesStore } from "@/stores/gamePackages";

const { t } = useI18n();
const gamePackages = useGamePackagesStore();

interface RegionSummary {
  group: string;
  displayName: string | null;
  numericId: string;
  plotCount: number;
}

interface RegionRender {
  pngBase64: string;
  width: number;
  height: number;
  originWorld?: [number, number];
  metersPerPixel: number;
  waterPlane: number;
  desert: boolean;
  displayName: string | null;
  plotCount: number;
  brushes: [string, [number, number][]][];
}

const selectedPackageId = ref<number | null>(null);
const regions = ref<RegionSummary[]>([]);
const selectedGroup = ref("");
const render = ref<RegionRender | null>(null);
const loadingRegions = ref(false);
const loadingRender = ref(false);
const errorMsg = ref("");

const openedPackages = computed(() => gamePackages.opened.map((o) => o.package));

function packageName(packageId: number): string {
  const opened = gamePackages.opened.find(
    (entry) => entry.package.packageId === packageId,
  );
  return opened?.package.path.split(/[\\/]/).pop() ?? String(packageId);
}

function packagePathOf(packageId: number): string {
  return (
    gamePackages.opened.find((entry) => entry.package.packageId === packageId)
      ?.package.path ?? ""
  );
}

async function selectPackage(packageId: number | null) {
  selectedPackageId.value = packageId;
  regions.value = [];
  selectedGroup.value = "";
  render.value = null;
  errorMsg.value = "";
  if (packageId === null) return;
  loadingRegions.value = true;
  try {
    regions.value = await invoke<RegionSummary[]>("map_panel_list_regions", {
      packagePath: packagePathOf(packageId),
    });
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loadingRegions.value = false;
  }
}

// ── 视图状态（8 m/像素 @ zoom 1）──
const MPP = 8;
/** 32 km 级区域所需的米刻度步长族。 */
const METER_STEPS = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
const wrap = ref<HTMLDivElement | null>(null);
const zoom = ref(1);
const pan = ref({ x: 0, y: 0 });
const panning = ref(false);
const cursorWorld = ref<{ x: number; y: number } | null>(null);
const showPlots = ref(true);
const showResources = ref(true);
let spaceDown = false;
let dragMode: "none" | "pan" = "none";
let panStart = { x: 0, y: 0, px: 0, py: 0 };

const xTicks = computed(() => buildTicks(false));
const yTicks = computed(() => buildTicks(true));

function buildTicks(vertical: boolean): { pos: number; label: string }[] {
  const total = (vertical ? render.value?.height : render.value?.width) ?? 0;
  if (!total) return [];
  const step =
    METER_STEPS.find((candidate) => (candidate / MPP) * zoom.value >= 72) ??
    METER_STEPS[METER_STEPS.length - 1];
  const viewSize = vertical
    ? (wrap.value?.clientHeight ?? 600)
    : (wrap.value?.clientWidth ?? 800);
  const ticks: { pos: number; label: string }[] = [];
  for (let meters = 0; meters <= total * MPP + 0.001; meters += step) {
    const pos = pan.value[vertical ? "y" : "x"] + (meters / MPP) * zoom.value;
    if (pos < -48 || pos > viewSize + 48) continue;
    ticks.push({ pos, label: `${Number(meters.toFixed(2))}m` });
  }
  return ticks;
}

function setZoom(value: number) {
  zoom.value = Math.min(32, Math.max(0.05, value));
}

function fit() {
  const element = wrap.value;
  if (!element || !render.value) return;
  const ratio = Math.min(
    (element.clientWidth - 64) / render.value.width,
    (element.clientHeight - 64) / render.value.height,
  );
  setZoom(Math.max(0.05, ratio));
  pan.value = {
    x: (element.clientWidth - render.value.width * zoom.value) / 2,
    y: (element.clientHeight - render.value.height * zoom.value) / 2,
  };
}

watch(render, () => fit());

// ── 指针交互（对齐 RasterCanvas：Pointer 事件 + capture，中键/空格平移）──

function onPointerDown(event: PointerEvent) {
  if (event.button === 1 || spaceDown) {
    dragMode = "pan";
    panning.value = true;
    panStart = {
      x: event.clientX,
      y: event.clientY,
      px: pan.value.x,
      py: pan.value.y,
    };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    event.preventDefault();
  }
}

function onPointerMove(event: PointerEvent) {
  const rect = wrap.value?.getBoundingClientRect();
  if (rect) {
    const px = (event.clientX - rect.left - pan.value.x) / zoom.value;
    const py = (event.clientY - rect.top - pan.value.y) / zoom.value;
    const size = render.value?.width ?? 0;
    cursorWorld.value =
      px >= 0 && py >= 0 && px <= size && py <= size
        ? {
            x: Math.round((render.value?.originWorld?.[0] ?? -16384) + px * MPP),
            y: Math.round((render.value?.originWorld?.[1] ?? -16384) + py * MPP),
          }
        : null;
  }
  if (dragMode === "pan") {
    pan.value = {
      x: panStart.px + (event.clientX - panStart.x),
      y: panStart.py + (event.clientY - panStart.y),
    };
  }
}

function onPointerUp() {
  dragMode = "none";
  panning.value = false;
}

function onWheel(event: WheelEvent) {
  event.preventDefault();
  setZoom(event.deltaY < 0 ? zoom.value * 1.15 : zoom.value / 1.15);
}

function onKeydown(event: KeyboardEvent) {
  if (event.code === "Space") spaceDown = true;
}
function onKeyup(event: KeyboardEvent) {
  if (event.code === "Space") spaceDown = false;
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("keyup", onKeyup);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("keyup", onKeyup);
});

// ── 数据加载 ──

const imgUrl = computed(() =>
  render.value ? `data:image/png;base64,${render.value.pngBase64}` : "",
);

const cursorText = computed(() => {
  if (!cursorWorld.value) return "";
  return `${cursorWorld.value.x}, ${cursorWorld.value.y} m`;
});

async function renderRegion() {
  const packageId = selectedPackageId.value;
  if (packageId === null || !selectedGroup.value) return;
  errorMsg.value = "";
  try {
    loadingRender.value = true;
    render.value = await invoke<RegionRender>("map_panel_render_region", {
      packagePath: packagePathOf(packageId),
      group: selectedGroup.value,
    });
    fit();
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loadingRender.value = false;
  }
}
</script>

<template>
  <section class="map-page">
    <header class="page-header">
      <span class="page-icon"><FIcon name="Map" :size="20" /></span>
      <div>
        <FTypography :header="2" spacing="none">{{
          t("studio.map.title")
        }}</FTypography>
        <p class="page-meta">{{ t("studio.map.meta") }}</p>
      </div>
    </header>

    <!-- 工具行：包选择 + 区域选择 + 渲染 -->
    <section class="toolbar" :aria-label="t('studio.map.configLabel')">
      <FDropdown :width="320">
        <template #trigger>
          <button
            type="button"
            class="package-trigger"
            :disabled="!openedPackages.length"
          >
            <FIcon name="Package" :size="14" />
            <span>{{
              selectedPackageId === null
                ? t("studio.map.selectPackage")
                : packageName(selectedPackageId)
            }}</span>
            <FIcon name="ChevronDown" :size="12" />
          </button>
        </template>
        <button
          v-for="pkg in openedPackages"
          :key="pkg.packageId"
          type="button"
          @click="selectPackage(pkg.packageId)"
        >
          <FIcon
            :name="selectedPackageId === pkg.packageId ? 'Check' : 'Package'"
            :size="14"
          />
          {{ packageName(pkg.packageId) }}
        </button>
        <div v-if="!openedPackages.length" class="menu-empty">
          {{ t("studio.map.needPackage") }}
        </div>
      </FDropdown>

      <FDropdown :width="300">
        <template #trigger>
          <button
            type="button"
            class="package-trigger"
            :disabled="!regions.length"
          >
            <span>{{
              selectedGroup
                ? (regions.find((r) => r.group === selectedGroup)
                    ?.displayName ?? selectedGroup)
                : t("studio.map.regionPlaceholder")
            }}</span>
            <FIcon name="ChevronDown" :size="12" />
          </button>
        </template>
        <button
          v-for="r in regions"
          :key="r.group"
          type="button"
          @click="selectedGroup = r.group"
        >
          <FIcon
            :name="selectedGroup === r.group ? 'Check' : 'MapPin'"
            :size="14"
          />
          {{ r.displayName ?? r.group }}（{{ r.plotCount }}）
        </button>
        <div v-if="!regions.length" class="menu-empty">
          {{ t("studio.map.emptyRegions") }}
        </div>
      </FDropdown>

      <button
        type="button"
        class="run-button"
        :disabled="!selectedGroup || loadingRender"
        @click="renderRegion"
      >
        <FIcon name="Play" :size="14" />
        {{ loadingRender ? t("studio.map.rendering") : t("studio.map.render") }}
      </button>
    </section>

    <p v-if="errorMsg" class="error-message" role="alert">{{ errorMsg }}</p>
    <p v-if="!openedPackages.length" class="notice" role="status">
      {{ t("studio.map.needPackage") }}
    </p>

    <!-- 主区：左预览 + 右面板 -->
    <div class="workspace">
      <div ref="wrap" class="viewer" :class="{ panning }">
        <div
          class="viewport"
          @wheel="onWheel"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
          @pointercancel="onPointerUp"
        >
          <div
            class="map-plane"
            :style="{
              transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
            }"
          >
            <img v-if="imgUrl" :src="imgUrl" draggable="false" class="map-img" />
          </div>
          <div v-if="!imgUrl" class="empty-hint">
            <FIcon name="Map" :size="28" />
            {{ t("studio.map.emptyPreview") }}
          </div>
        </div>
        <!-- 米制标尺 -->
        <div v-if="render" class="ruler ruler-x" aria-hidden="true">
          <span
            v-for="tick in xTicks"
            :key="`x${tick.label}`"
            class="ruler-tick"
            :style="{ left: `${tick.pos}px` }"
            >{{ tick.label }}</span
          >
        </div>
        <div v-if="render" class="ruler ruler-y" aria-hidden="true">
          <span
            v-for="tick in yTicks"
            :key="`y${tick.label}`"
            class="ruler-tick"
            :style="{ top: `${tick.pos}px` }"
            >{{ tick.label }}</span
          >
        </div>
        <span v-if="render" class="scale-badge">
          1 px = {{ render.metersPerPixel }} m
        </span>
        <!-- HUD -->
        <div v-if="render" class="hud">
          <span v-if="cursorWorld" class="hud-item mono">{{ cursorText }}</span>
          <span class="hud-item">{{ render.width }}×{{ render.height }}</span>
          <span class="hud-item">{{ Math.round(zoom * 100) }}%</span>
          <button
            class="hud-button"
            type="button"
            :title="t('studio.raster.fit')"
            @click="fit"
          >
            <FIcon name="Maximize" :size="12" aria-label="" />
          </button>
          <button
            class="hud-button"
            type="button"
            :title="t('studio.raster.zoom100')"
            @click="setZoom(1)"
          >
            1:1
          </button>
        </div>
        <span v-if="render" class="pan-hint">{{
          t("studio.map.panHint")
        }}</span>
      </div>

      <aside class="side-panel">
        <section class="side-section">
          <h3>
            {{ render?.displayName ?? t("studio.map.propertiesTitle") }}
          </h3>
          <dl v-if="render" class="props">
            <dt>{{ t("studio.map.sizeLabel") }}</dt>
            <dd>{{ render.width }}×{{ render.height }}</dd>
            <dt>{{ t("studio.map.originWorld") }}</dt>
            <dd class="mono">
              {{ render.originWorld?.[0]?.toFixed(0) ?? "?" }},
              {{ render.originWorld?.[1]?.toFixed(0) ?? "?" }}
            </dd>
            <dt>{{ t("studio.map.waterPlane") }}</dt>
            <dd>{{ render.waterPlane }}</dd>
            <dt>{{ t("studio.map.desertMode") }}</dt>
            <dd>{{ render.desert ? t("studio.map.yes") : t("studio.map.no") }}</dd>
            <dt>{{ t("studio.map.plotCount") }}</dt>
            <dd>{{ render.plotCount }}</dd>
            <dt>{{ t("studio.map.brushCount") }}</dt>
            <dd>{{ render.brushes.length }}</dd>
          </dl>
          <p v-else class="side-empty">{{ t("studio.map.emptySide") }}</p>
        </section>

        <section class="side-section">
          <h3>{{ t("studio.map.layersTitle") }}</h3>
          <label class="layer-toggle">
            <input v-model="showPlots" type="checkbox" />
            {{ t("studio.map.layerPlots") }}
          </label>
          <label class="layer-toggle">
            <input v-model="showResources" type="checkbox" />
            {{ t("studio.map.layerResources") }}
          </label>
          <p class="side-note">{{ t("studio.map.layersNote") }}</p>
        </section>

        <p class="side-foot">{{ t("studio.map.pipelineNote") }}</p>
      </aside>
    </div>
  </section>
</template>

<style scoped>
.map-page {
  padding-bottom: 3rem;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
  height: 100%;
  min-height: 0;
}
.page-header {
  display: flex;
  align-items: center;
  gap: 1rem;
}
.page-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--brand);
  flex-shrink: 0;
}
.page-header :deep(h2) {
  margin: 0;
}
.page-meta {
  margin: 0.2rem 0 0;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
}
.package-trigger {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.45rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--foreground);
  font-size: 0.75rem;
  cursor: pointer;
}
.package-trigger:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.run-button {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.45rem 0.9rem;
  border: 1px solid var(--brand);
  border-radius: var(--radius-md);
  background: var(--brand);
  color: var(--brand-foreground, #fff);
  font-size: 0.75rem;
  cursor: pointer;
}
.run-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.error-message,
.notice {
  margin: 0;
  padding: 0.7rem 1rem;
  border-radius: var(--radius-md);
  font-size: 0.75rem;
}
.error-message {
  border: 1px solid var(--destruct, #b91c1c);
  color: var(--destruct, #b91c1c);
  background: color-mix(in srgb, var(--destruct, #b91c1c) 8%, transparent);
}
.notice {
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  background: var(--surface);
}
.workspace {
  display: flex;
  gap: 0.75rem;
  flex: 1;
  min-height: 480px;
}
/* 查看器（对齐 RasterCanvas：点阵底 + 标尺 + HUD） */
.viewer {
  background: var(--surface);
  background-image: radial-gradient(
    circle at 1px 1px,
    color-mix(in srgb, var(--border) 60%, transparent) 1px,
    transparent 0
  );
  background-size: 16px 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  cursor: crosshair;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  position: relative;
}
.viewer.panning {
  cursor: grab;
}
.viewport {
  position: absolute;
  inset: 0;
  overflow: hidden;
}
.map-plane {
  position: absolute;
  top: 0;
  left: 0;
  transform-origin: 0 0;
}
.map-img {
  display: block;
  user-select: none;
  pointer-events: none;
}
.empty-hint {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.6rem;
  color: var(--subtle-foreground);
  font-size: 0.8125rem;
}
.ruler {
  position: absolute;
  pointer-events: none;
  z-index: 2;
}
.ruler-x {
  background: color-mix(in srgb, var(--surface) 80%, transparent);
  border-bottom: 1px solid var(--border);
  height: 18px;
  left: 0;
  right: 0;
  top: 0;
}
.ruler-y {
  background: color-mix(in srgb, var(--surface) 80%, transparent);
  border-right: 1px solid var(--border);
  bottom: 0;
  left: 0;
  top: 0;
  width: 44px;
}
.ruler-tick {
  color: var(--muted-foreground);
  font-size: 9.5px;
  font-variant-numeric: tabular-nums;
  position: absolute;
  white-space: nowrap;
}
.ruler-x .ruler-tick {
  border-left: 1px solid
    color-mix(in srgb, var(--border-strong, var(--border)) 70%, transparent);
  height: 100%;
  padding: 2px 0 0 3px;
}
.ruler-y .ruler-tick {
  border-top: 1px solid
    color-mix(in srgb, var(--border-strong, var(--border)) 70%, transparent);
  height: 0;
  padding: 0 2px;
  transform: translateY(-7px);
  width: max-content;
}
.scale-badge {
  background: color-mix(in srgb, var(--surface) 85%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  font-size: 10px;
  padding: 2px 8px;
  position: absolute;
  right: 12px;
  top: 24px;
  z-index: 3;
}
.hud {
  bottom: 8px;
  display: flex;
  gap: 6px;
  position: absolute;
  right: 8px;
  z-index: 3;
}
.hud-item,
.hud-button {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 85%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 10.5px;
  padding: 3px 8px;
}
.hud-button {
  cursor: pointer;
}
.hud-button:hover {
  color: var(--foreground);
}
.mono {
  font-family: var(--font-mono, ui-monospace, monospace);
}
.pan-hint {
  bottom: 8px;
  color: var(--subtle-foreground);
  font-size: 10px;
  left: 52px;
  position: absolute;
  z-index: 2;
}
.side-panel {
  width: 272px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
  overflow-y: auto;
}
.side-section {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  padding: 0.85rem 1rem;
}
.side-section h3 {
  margin: 0 0 0.6rem;
  font-size: 0.8125rem;
  font-weight: 600;
}
.props {
  margin: 0;
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 0.35rem 0.8rem;
  font-size: 0.75rem;
}
.props dt {
  color: var(--muted-foreground);
}
.props dd {
  margin: 0;
  text-align: right;
  font-family: var(--font-mono, ui-monospace, monospace);
}
.mono {
  font-family: var(--font-mono, ui-monospace, monospace);
}
.side-empty {
  margin: 0;
  font-size: 0.75rem;
  color: var(--subtle-foreground);
}
.layer-toggle {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.75rem;
  padding: 0.25rem 0;
  cursor: pointer;
}
.side-note {
  margin: 0.5rem 0 0;
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
}
.side-foot {
  margin: auto 0 0;
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
  font-family: var(--font-mono, ui-monospace, monospace);
}
.menu-empty {
  padding: 0.5rem 0.75rem;
  font-size: 0.75rem;
  color: var(--muted-foreground);
}
</style>
