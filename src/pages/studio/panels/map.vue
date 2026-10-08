<script setup lang="ts">
/**
 * 地图开发面板：区域地形 3D 预览。
 * 左 = 地图预览卡片（工具栏：全屏检查），右 = 属性与图层区（布局沿用 v1）。
 * 全屏 sheet 复用同一 MapViewer3D，便于更细致的地图检查。
 * 渲染管线：sc_properties::region_3d（341-tile 金字塔 + ED 地面场 +
 * 水位面 4928(-870m) + 地块名/伟工位；three.js 位移网格 + 引擎式顶点色）。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { hasStateResource } from "@/components/map/map-state-fields";
import { invoke } from "@tauri-apps/api/core";
import FIcon from "@/components/extensions/FIcon.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FCheckbox from "@/components/ui/FCheckbox.vue";
import FSheet from "@/components/ui/FSheet.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import MapResourceControls from "@/components/map/MapResourceControls.vue";
import MapViewer3D from "@/components/map/MapViewer3D.vue";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { brushResourceKind } from "@/lib/region-map";
import { ecoChannel } from "@/components/map/map-fields";
import type { Region3DData, RegionSummary } from "@/lib/region-map";

const { t, te, locale } = useI18n();
const gamePackages = useGamePackagesStore();

const saveDirectory = ref<string | null>(null);
function resourceAvailable(kind: string) {
  return ecoChannel(kind) !== null || hasStateResource(render.value, kind);
}
async function chooseSave() {
  const path = await open({
    directory: true,
    multiple: false,
    title: t("studio.map.loadSave"),
  });
  if (typeof path !== "string") return;
  saveDirectory.value = path;
  await renderRegion();
}
const selectedPackageId = ref<number | null>(null);
const regions = ref<RegionSummary[]>([]);
const selectedGroup = ref("");
const render = ref<Region3DData | null>(null);
const loadingRegions = ref(false);
const loadingRender = ref(false);
const errorMsg = ref("");
const sheetOpen = ref(false);

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
const selectedRegionName = computed(
  () =>
    regions.value
      .filter((r) => r.group === selectedGroup.value)
      .map(regionDisplayName)[0] ?? selectedGroup.value,
);

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
  saveDirectory.value = null;
  render.value = null;
  activeResource.value = null;
  errorMsg.value = "";
}
async function selectPackage(packageId: number | null) {
  const request = ++regionsRequest;
  ++renderRequest;
  loadingRender.value = false;
  loadingRegions.value = false;
  activeResource.value = null;
  saveDirectory.value = null;
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
  activeResource.value = null;
  try {
    loadingRender.value = true;
    const result = await invoke<Region3DData>("map_panel_region_3d", {
      packagePath: packagePathOf(packageId),
      group: selectedGroup.value,
      saveDirectory: saveDirectory.value,
    });
    if (request === renderRequest) render.value = result;
  } catch (e) {
    if (request === renderRequest) errorMsg.value = String(e);
  } finally {
    if (request === renderRequest) loadingRender.value = false;
  }
}

// ── 图层状态：地块框开关 + 资源分布图层单选（null = 关闭；对齐游戏数据视图）──
const showPlots = ref(true);
const showVegetation = ref(true);
const showWater = ref(true);
const showRoads = ref(true);
const resourceOpacity = ref(0.75);
const brushes = computed(() => render.value?.brushes ?? []);
const resourceKinds = computed(() => {
  const kinds: string[] = render.value
    ? ["soil", "waterTable", "forest", "coal", "ore", "oil"]
    : [];
  for (const [name] of brushes.value) {
    const kind = brushResourceKind(name);
    if (!kinds.some((k) => k.toLowerCase() === kind.toLowerCase()))
      kinds.push(kind);
  }
  return kinds;
});
const activeResource = ref<string | null>(null);
const viewerLayers = computed(() => ({
  showPlots: showPlots.value,
  showVegetation: showVegetation.value,
  showWater: showWater.value,
  showRoads: showRoads.value,
  activeResource: activeResource.value,
  resourceOpacity: resourceOpacity.value,
}));
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
      <button
        type="button"
        class="package-trigger"
        :disabled="!render || loadingRender"
        @click="chooseSave"
        :title="
          render?.referenceDirectory ??
          saveDirectory ??
          t('studio.map.loadSave')
        "
      >
        <FIcon name="FolderOpen" :size="14" />{{
          t(
            render?.referenceDirectory || saveDirectory
              ? "studio.map.changeSave"
              : "studio.map.loadSave",
          )
        }}
      </button>
    </section>

    <p v-if="errorMsg" class="error-message" role="alert">{{ errorMsg }}</p>
    <p v-if="render" class="notice" role="status">
      {{
        t(
          render.roadAssets
            ? "studio.map.referenceLoaded"
            : "studio.map.referenceMissing",
        )
      }}
    </p>
    <p v-if="!openedPackages.length" class="notice" role="status">
      {{ t("studio.map.needPackage") }}
    </p>

    <!-- 主区：左预览卡片 + 右面板 -->
    <div class="workspace">
      <div class="viewer-card">
        <!-- 顶部工具条：横向 flex 右对齐（分段式图层切换 + outline 展开） -->
        <div class="card-toolbar">
          <MapResourceControls
            v-model="activeResource"
            v-model:opacity="resourceOpacity"
            :options="resourceOptions"
            :disabled="!render || loadingRender"
          />
          <button
            type="button"
            class="outline-btn"
            :title="t('studio.map.fullscreen')"
            :disabled="!render"
            @click="sheetOpen = true"
          >
            <FIcon name="Expand" :size="15" aria-label="" />
          </button>
        </div>
        <div class="card-viewer">
          <MapViewer3D :data="render" v-bind="viewerLayers" />
          <div v-if="!render && !loadingRender" class="viewer-empty">
            <FEmpty
              icon-name="Map"
              variant="compact"
              status="default"
              :title="t('studio.map.emptyTitle')"
              :desc="t('studio.map.emptyDescription')"
            />
          </div>
        </div>
      </div>

      <aside class="side-panel">
        <section class="side-section">
          <h3>
            {{ renderRegionName || t("studio.map.propertiesTitle") }}
          </h3>
          <dl v-if="render" class="props">
            <dt>{{ t("studio.map.sizeLabel") }}</dt>
            <dd>{{ render.size }}×{{ render.size }}</dd>
            <dt>{{ t("studio.map.originWorld") }}</dt>
            <dd class="mono">
              {{ render.originWorld[0].toFixed(0) }},
              {{ render.originWorld[1].toFixed(0) }}
            </dd>
            <dt>{{ t("studio.map.waterPlane") }}</dt>
            <dd>{{ Math.round((render.waterZ + 1024) * 32) }}</dd>
            <dt>{{ t("studio.map.desertMode") }}</dt>
            <dd>
              {{ render.desert ? t("studio.map.yes") : t("studio.map.no") }}
            </dd>
            <dt>{{ t("studio.map.plotCount") }}</dt>
            <dd>{{ render.plots.filter((p) => p.kind === "city").length }}</dd>
            <dt>{{ t("studio.map.brushCount") }}</dt>
            <dd>{{ render.brushes.length }}</dd>
          </dl>
          <p v-else class="side-empty">{{ t("studio.map.emptySide") }}</p>
        </section>

        <section class="side-section">
          <h3>{{ t("studio.map.resourcesTitle") }}</h3>
          <ul v-if="brushes.length" class="brush-list">
            <li v-for="[name, stamps] in brushes" :key="name">
              {{ name }} · {{ stamps.length }}
            </li>
          </ul>
          <p v-else class="side-empty">{{ t("studio.map.emptySide") }}</p>
        </section>

        <section class="side-section">
          <h3>{{ t("studio.map.layersTitle") }}</h3>
          <label class="layer-toggle">
            <FCheckbox v-model="showPlots" />
            {{ t("studio.map.layerPlots") }}
          </label>
          <label class="layer-toggle">
            <FCheckbox v-model="showVegetation" />
            {{ t("studio.map.layerVegetation") }}
          </label>
          <label class="layer-toggle">
            <FCheckbox v-model="showWater" />
            {{ t("studio.map.layerWater") }}
          </label>
          <label class="layer-toggle">
            <FCheckbox v-model="showRoads" />
            {{ t("studio.map.layerRoads") }}
          </label>
          <p class="side-note">{{ t("studio.map.layersNote") }}</p>
        </section>

        <p class="side-foot">{{ t("studio.map.pipelineNote") }}</p>
      </aside>
    </div>

    <!-- 全屏检查 sheet：复用同一渲染与查看器 -->
    <FSheet :open="sheetOpen" width="100vw" @update:open="sheetOpen = $event">
      <div class="sheet-body">
        <header class="sheet-header">
          <span class="page-icon"><FIcon name="Map" :size="18" /></span>
          <div class="sheet-title">
            {{ renderRegionName || selectedRegionName }}
            <span class="sheet-sub">{{ t("studio.map.fullscreen") }}</span>
          </div>
          <MapResourceControls
            v-model="activeResource"
            v-model:opacity="resourceOpacity"
            :options="resourceOptions"
            :disabled="!render || loadingRender"
          />
          <button
            type="button"
            class="outline-btn"
            :aria-label="t('shell.close')"
            @click="sheetOpen = false"
          >
            <FIcon name="X" :size="15" aria-label="" />
          </button>
        </header>
        <div class="sheet-viewer">
          <MapViewer3D v-if="sheetOpen" :data="render" v-bind="viewerLayers" />
        </div>
      </div>
    </FSheet>
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
/* 预览卡片：顶部横向工具条（分段式图层切换 + outline 按钮） */
.viewer-card {
  flex: 1;
  min-width: 0;
  min-height: 480px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--surface);
  display: flex;
  flex-direction: column;
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
.brush-list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  font-size: 0.75rem;
  font-family: var(--font-mono, ui-monospace, monospace);
  color: var(--muted-foreground);
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
/* 全屏检查 sheet */
.sheet-body {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 0.75rem;
  padding: 0.9rem 1.1rem;
}
.sheet-header {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.75rem;
}
.sheet-title {
  flex: 1;
  font-size: 0.9rem;
  font-weight: 600;
}
.sheet-sub {
  margin-left: 0.5rem;
  font-size: 0.6875rem;
  font-weight: 400;
  color: var(--subtle-foreground);
  font-family: var(--font-mono, ui-monospace, monospace);
}
/* 对齐 property editor 的 outline 图标按钮（见 .outline-btn） */
.sheet-viewer {
  flex: 1;
  min-height: 0;
}
</style>
