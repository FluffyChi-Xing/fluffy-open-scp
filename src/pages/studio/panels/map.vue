<script setup lang="ts">
/**
 * 地图开发面板：区域地形 3D 预览。
 * 左 = 地图预览卡片（工具栏：全屏检查），右 = 属性与图层区（布局沿用 v1）。
 * 全屏 sheet 复用同一 MapViewer3D，便于更细致的地图检查。
 * 渲染管线：sc_properties::region_3d（341-tile 金字塔 + ED 地面场 +
 * 水位面 4928(-870m) + 地块名/伟工位；three.js 位移网格 + 引擎式顶点色）。
 */
import { computed, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { hasStateResource } from "@/components/map/map-state-fields";
import { invoke } from "@tauri-apps/api/core";
import FIcon from "@/components/extensions/FIcon.vue";
import MapPlotDetails from "@/components/map/MapPlotDetails.vue";
import FDropdown from "@/components/ui/FDropdown.vue";

import FTypography from "@/components/extensions/FTypography.vue";

import MapPreviewPane from "@/components/map/MapPreviewPane.vue";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { ecoChannel } from "@/components/map/map-fields";
import type { Region3DData, RegionSummary } from "@/lib/region-map";

const { t, te, locale } = useI18n();
const gamePackages = useGamePackagesStore();

function resourceAvailable(kind: string) {
  return ecoChannel(kind) !== null || hasStateResource(render.value, kind);
}
const selectedPackageId = ref<number | null>(null);
const regions = ref<RegionSummary[]>([]);
const selectedGroup = ref("");
const render = shallowRef<Region3DData | null>(null);
const loadingRegions = ref(false);
const loadingRender = ref(false);
const errorMsg = ref("");

/** 按当前 UI 语言取区域名（zh → 中文名，否则英文名），缺失回退另一语言/组 id。 */
function regionDisplayName(r: {
  displayName: string | null;
  displayNameEn: string | null;
  group: string;
}): string {
  const zh = locale.value.startsWith("zh");
  const primary = zh ? r.displayName : r.displayNameEn;
  const fallback = zh ? r.displayNameEn : r.displayName;
  return primary ?? fallback ?? r.group;
}

const openedPackages = computed(() =>
  gamePackages.opened.map((o) => o.package),
);
/** 渲染结果的区域名（跟随 UI 语言）。 */
const renderRegionName = computed(() => {
  const r = render.value;
  if (!r) return "";
  const zh = locale.value.startsWith("zh");
  return (
    (zh ? r.displayName : r.displayNameEn) ??
    r.displayName ??
    r.displayNameEn ??
    ""
  );
});
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

let renderRequest = 0;
let regionsRequest = 0;
function selectRegion(group: string) {
  if (group === selectedGroup.value) return;
  ++renderRequest;
  loadingRender.value = false;
  selectedGroup.value = group;
  render.value = null;

  selectedPlotUid.value = null;
  errorMsg.value = "";
}
async function selectPackage(packageId: number | null) {
  const request = ++regionsRequest;
  ++renderRequest;
  loadingRender.value = false;
  loadingRegions.value = false;

  selectedPlotUid.value = null;
  selectedPackageId.value = packageId;
  regions.value = [];
  selectedGroup.value = "";
  render.value = null;
  errorMsg.value = "";
  if (packageId === null) return;
  loadingRegions.value = true;
  try {
    const result = await invoke<RegionSummary[]>("map_panel_list_regions", {
      packagePath: packagePathOf(packageId),
    });
    if (request === regionsRequest) regions.value = result;
  } catch (e) {
    if (request === regionsRequest) errorMsg.value = String(e);
  } finally {
    if (request === regionsRequest) loadingRegions.value = false;
  }
}

async function renderRegion() {
  const packageId = selectedPackageId.value;
  if (packageId === null || !selectedGroup.value) return;
  const request = ++renderRequest;
  errorMsg.value = "";

  try {
    loadingRender.value = true;
    const result = await invoke<Region3DData>("map_panel_region_3d", {
      packagePath: packagePathOf(packageId),
      group: selectedGroup.value,
    });
    if (request === renderRequest) {
      render.value = result;
      selectedPlotUid.value = null;
    }
  } catch (e) {
    if (request === renderRequest) errorMsg.value = String(e);
  } finally {
    if (request === renderRequest) loadingRender.value = false;
  }
}

// ── 图层状态：地块框开关 + 资源分布图层单选（null = 关闭；对齐游戏数据视图）──

const resourceKinds = computed(() =>
  render.value ? ["soil", "waterTable", "forest", "coal", "ore", "oil"] : [],
);
const selectedPlotUid = ref<string | null>(null);
const selectedPlot = computed(
  () =>
    render.value?.plots.find((plot) => plot.uid === selectedPlotUid.value) ??
    null,
);
const resourceOptions = computed(() =>
  resourceKinds.value.map((kind) => ({
    kind,
    label: resourceLabel(kind),
    available: resourceAvailable(kind),
  })),
);
function resourceLabel(kind: string): string {
  const key = `studio.map.res${kind.charAt(0).toUpperCase()}${kind.slice(1)}`;
  return te(key) ? t(key) : kind;
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
                ? (regions
                    .filter((r) => r.group === selectedGroup)
                    .map(regionDisplayName)[0] ?? selectedGroup)
                : t("studio.map.regionPlaceholder")
            }}</span>
            <FIcon name="ChevronDown" :size="12" />
          </button>
        </template>
        <button
          v-for="r in regions"
          :key="r.group"
          type="button"
          @click="selectRegion(r.group)"
        >
          <FIcon
            :name="selectedGroup === r.group ? 'Check' : 'MapPin'"
            :size="14"
          />
          {{ regionDisplayName(r) }}（{{ r.plotCount }}）
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
    <p v-if="render" class="notice" role="status">
      {{
        t(
          `studio.map.static${render.staticStatus === "ready" ? "Ready" : render.staticStatus === "invalid" ? "Invalid" : "Missing"}`,
        )
      }}
      <span v-if="render.staticStatus === 'ready' && !render.roadAssets">{{
        t("studio.map.roadAssetsMissing")
      }}</span>
    </p>
    <p v-if="!openedPackages.length" class="notice" role="status">
      {{ t("studio.map.needPackage") }}
    </p>

    <!-- 主区：左预览卡片 + 右面板 -->
    <div class="workspace">
      <div class="viewer-card">
        <MapPreviewPane
          :data="render"
          :title="renderRegionName"
          :options="resourceOptions"
          :selected-plot-uid="selectedPlotUid"
          @select-plot="selectedPlotUid = $event"
        />
      </div>
      <aside class="side-panel">
        <MapPlotDetails :data="render" :plot="selectedPlot" />
      </aside>
    </div>
  </section>
</template>

<style scoped>
.map-page {
  box-sizing: border-box;
  padding-bottom: 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
  height: 100%;
  max-height: calc(100dvh - 7.5rem);
  min-height: 0;
  overflow: hidden;
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
  min-height: 0;
  overflow: hidden;
}
/* 预览卡片：顶部横向工具条（分段式图层切换 + outline 按钮） */
.viewer-card {
  flex: 1;
  min-width: 0;
  min-height: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--surface);
  display: flex;
  flex-direction: column;
}
:deep(.map-preview-pane) {
  min-height: 0;
}
.card-toolbar {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: flex-end;
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}
/* outline 图标按钮（展开/关闭，对齐 property editor） */
.outline-btn {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 28px;
  min-width: 28px;
}
.outline-btn:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.outline-btn:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.card-viewer {
  position: relative;
  flex: 1;
  min-height: 0;
}
.viewer-empty {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 1.5rem;
}
.viewer-empty :deep(.f-empty) {
  max-width: 22rem;
}
.side-panel {
  width: 272px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
  overflow-y: auto;
}
.menu-empty {
  padding: 0.5rem 0.75rem;
  font-size: 0.75rem;
  color: var(--muted-foreground);
}
</style>
