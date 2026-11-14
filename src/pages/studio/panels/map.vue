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

const terrainSource = computed(() => {
  const id = selectedPackageId.value;
  if (id === null || !selectedGroup.value) return null;
  const packagePath = packagePathOf(id);
  return packagePath ? { packagePath, group: selectedGroup.value } : null;
});

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
  render.value
    ? ["soil", "waterTable", "cityWaterTable", "forest", "coal", "ore", "oil"]
    : [],
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
    <header class="map-command-bar">
      <h2 class="map-heading"><FIcon name="Map" :size="19" />{{ t("studio.map.title") }}</h2>
      <div class="toolbar" role="group" :aria-label="t('studio.map.configLabel')">
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
      </div>
    </header>

    <p v-if="errorMsg" class="error-message" role="alert">{{ errorMsg }}</p>
    <p v-if="render && (render.staticStatus !== 'ready' || !render.roadAssets)" class="notice" role="status">
      {{ t(render.staticStatus === "ready"
        ? "studio.map.roadAssetsMissing"
        : render.staticStatus === "invalid" ? "studio.map.staticInvalid" : "studio.map.staticMissing") }}
    </p>
    <div class="workspace" :class="{ 'has-inspector': render }">
      <div class="viewer-card">
        <MapPreviewPane
          :data="render"
          :title="renderRegionName"
          :empty-hint="!openedPackages.length ? t('studio.map.needPackage') : undefined"
          :options="resourceOptions"
          :terrain-source="terrainSource"
          :selected-plot-uid="selectedPlotUid"
          @select-plot="selectedPlotUid = $event"
        />
      </div>
      <aside v-if="render" class="side-panel">
        <MapPlotDetails :data="render" :plot="selectedPlot" />
      </aside>
    </div>
  </section>
</template>

<style scoped>
.map-page {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 12px;
  block-size: calc(100dvh - 7.5rem);
  min-block-size: 34rem;
  padding: 2px 2px 10px;
}
.map-command-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px 24px;
  padding: 8px 2px 11px;
  border-block-end: 1px solid var(--border);
}
.map-heading {
  display: flex;
  align-items: center;
  gap: 9px;
  margin: 0;
  font-size: 1.0625rem;
  font-weight: 650;
}
.map-heading :deep(svg) { color: var(--brand); }
.toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  min-inline-size: 0;
}
.package-trigger, .run-button {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-block-size: 34px;
  max-inline-size: min(21rem, 80vw);
  padding-inline: 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--foreground);
  font-size: .75rem;
  cursor: pointer;
}
.package-trigger span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.package-trigger:hover:enabled { background: var(--surface-hover); }
.run-button {
  padding-inline: 14px;
  border-color: var(--brand);
  background: var(--brand);
  color: var(--brand-foreground, #fff);
}
.package-trigger:disabled, .run-button:disabled { opacity: .48; cursor: not-allowed; }
.package-trigger:focus-visible, .run-button:focus-visible {
  outline: 2px solid var(--brand);
  outline-offset: 2px;
}
.error-message, .notice {
  margin: 0;
  padding: 6px 10px;
  border-inline-start: 2px solid var(--border);
  color: var(--muted-foreground);
  font-size: .75rem;
}
.error-message {
  border-color: var(--destruct, #b91c1c);
  color: var(--destruct, #b91c1c);
}
.workspace {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  flex: 1;
  min-block-size: 0;
}
.workspace.has-inspector { grid-template-columns: minmax(0, 1fr) 17rem; gap: 12px; }
.viewer-card { min-inline-size: 0; min-block-size: 0; }
.side-panel { overflow-y: auto; min-inline-size: 0; }
.menu-empty { padding: .5rem .75rem; font-size: .75rem; color: var(--muted-foreground); }
@media (max-width: 900px) {
  .map-page { block-size: auto; }
  .workspace.has-inspector { grid-template-columns: minmax(0, 1fr); }
  .viewer-card { block-size: max(26rem, 68dvh); }
}
</style>
