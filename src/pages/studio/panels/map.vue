<script setup lang="ts">
/**
 * 地图开发面板：区域地形合成预览。
 * 左侧 = 可交互地图预览（滚轮缩放 / 中键拖动 / 米制标尺），
 * 右侧 = 区域属性与图层区（未来叠加资源多层视图与地图笔刷）。
 * 渲染管线：sc_properties::region_map（341-tile 金字塔 + 全局水位面 3336）。
 */
import { computed, ref } from "vue";
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

function regionLabel(group: string): string {
  const r = regions.value.find((x) => x.group === group);
  if (!r) return group;
  return r.displayName ?? `${group} · ${r.plotCount}`;
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

// 视图状态
const zoom = ref(1);
const pan = ref({ x: 0, y: 0 });
const panning = ref(false);
let panStart = { x: 0, y: 0, px: 0, py: 0 };
const showPlots = ref(true);
const showResources = ref(true);

const imgUrl = computed(() =>
  render.value ? `data:image/png;base64,${render.value.pngBase64}` : "",
);

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
    zoom.value = 1;
    pan.value = { x: 0, y: 0 };
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loadingRender.value = false;
  }
}

function onWheel(event: WheelEvent) {
  if (!render.value) return;
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const mx = event.clientX - rect.left;
  const my = event.clientY - rect.top;
  const factor = event.deltaY < 0 ? 1.15 : 1 / 1.15;
  const next = Math.min(12, Math.max(0.1, zoom.value * factor));
  pan.value.x = mx - ((mx - pan.value.x) * next) / zoom.value;
  pan.value.y = my - ((my - pan.value.y) * next) / zoom.value;
  zoom.value = next;
}

function onMouseDown(event: MouseEvent) {
  if (event.button === 1) {
    panning.value = true;
    panStart = { x: event.clientX, y: event.clientY, px: pan.value.x, py: pan.value.y };
    event.preventDefault();
  }
}
function onMouseMove(event: MouseEvent) {
  if (!panning.value) return;
  pan.value.x = panStart.px + (event.clientX - panStart.x);
  pan.value.y = panStart.py + (event.clientY - panStart.py);
}
function onMouseUp() {
  panning.value = false;
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
                ? regionLabel(selectedGroup)
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
          {{ regionLabel(r.group) }}
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
      <div class="viewer" :class="{ panning }">
        <div
          class="viewport"
          @wheel="onWheel"
          @mousedown="onMouseDown"
          @mousemove="onMouseMove"
          @mouseup="onMouseUp"
          @mouseleave="onMouseUp"
        >
          <img
            v-if="imgUrl"
            :src="imgUrl"
            draggable="false"
            class="map-img"
            :style="{
              transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
              transformOrigin: '0 0',
              imageRendering: zoom >= 3 ? ('pixelated' as const) : 'auto',
            }"
          />
          <div v-else class="empty-hint">
            <FIcon name="Map" :size="28" />
            {{ t("studio.map.emptyPreview") }}
          </div>
        </div>
        <div v-if="render" class="status-chip">
          {{ t("studio.map.metersPerPx", { n: (render.metersPerPixel / zoom).toFixed(1) }) }}
          · {{ t("studio.map.zoomPct", { n: Math.round(zoom * 100) }) }}
        </div>
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
  min-height: 0;
  flex: 1;
}
.viewer {
  position: relative;
  flex: 1;
  min-width: 0;
  min-height: 480px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  overflow: hidden;
}
.viewport {
  position: absolute;
  inset: 0;
  overflow: hidden;
  cursor: grab;
}
.viewport.panning {
  cursor: grabbing;
}
.map-img {
  position: absolute;
  user-select: none;
  pointer-events: none;
}
.empty-hint {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.6rem;
  color: var(--subtle-foreground);
  font-size: 0.8125rem;
}
.status-chip {
  position: absolute;
  right: 0.5rem;
  bottom: 0.5rem;
  padding: 0.25rem 0.6rem;
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--surface) 75%, transparent);
  border: 1px solid var(--border);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  color: var(--muted-foreground);
  pointer-events: none;
}
.side-panel {
  width: 272px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
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
