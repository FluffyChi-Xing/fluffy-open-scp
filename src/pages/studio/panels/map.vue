<script setup lang="ts">
/**
 * 地图开发面板：区域地形合成预览。
 * 左 = 地图预览卡片（工具栏：全屏检查），右 = 属性与图层区。
 * 全屏 sheet 复用同一 MapViewer，便于更细致的地图检查。
 * 渲染管线：sc_properties::region_map（341-tile 金字塔 + 全局水位面 3336）。
 */
import { computed, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FCheckbox from "@/components/ui/FCheckbox.vue";
import FSheet from "@/components/ui/FSheet.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import MapViewer from "@/components/map/MapViewer.vue";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { brushResourceKind } from "@/lib/region-map";
import type {
  BrushList,
  OverlayWriteResult,
  RegionRender,
  RegionSummary,
} from "@/lib/region-map";

const { t, te, locale } = useI18n();
const gamePackages = useGamePackagesStore();

const selectedPackageId = ref<number | null>(null);
const regions = ref<RegionSummary[]>([]);
const selectedGroup = ref("");
const render = ref<RegionRender | null>(null);
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

const openedPackages = computed(() => gamePackages.opened.map((o) => o.package));
/** 渲染结果的区域名（跟随 UI 语言）。 */
const renderRegionName = computed(() => {
  const r = render.value;
  if (!r) return "";
  const zh = locale.value.startsWith("zh");
  return (zh ? r.displayName : r.displayNameEn) ?? r.displayName ?? r.displayNameEn ?? "";
});
const selectedRegionName = computed(
  () =>
    regions.value
      .filter((r) => r.group === selectedGroup.value)
      .map(regionDisplayName)[0] ??
    selectedGroup.value,
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
    waterMetersInput.value = Math.round(render.value.waterPlane / 32 - 1024);
    await loadBrushLists();
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    loadingRender.value = false;
  }
}

// ── 图层状态：地块框开关 + 资源分布图层单选（null = 关闭；对齐游戏数据视图）──
const showPlots = ref(true);
const brushes = computed(() => render.value?.brushes ?? []);
const resourceKinds = computed(() => {
  const kinds: string[] = [];
  for (const [name] of brushes.value) {
    const kind = brushResourceKind(name);
    if (!kinds.some((k) => k.toLowerCase() === kind.toLowerCase())) kinds.push(kind);
  }
  return kinds;
});
const activeResource = ref<string | null>(null);
const visibleResourceKinds = computed(() =>
  activeResource.value ? [activeResource.value] : [],
);
function selectResource(kind: string) {
  activeResource.value = activeResource.value === kind ? null : kind;
}
function resourceLabel(kind: string): string {
  const key = `studio.map.res${kind.charAt(0).toUpperCase()}${kind.slice(1)}`;
  return te(key) ? t(key) : kind;
}

// ── 编辑（P1-5/P1-6）：全部走 overlay 副本，绝不触碰源包 ──
const brushLists = ref<BrushList[]>([]);
const selectedBrushInstance = ref("");
const placementMode = ref(false);
const pendingAdds = ref<[number, number][]>([]);
const pendingRemoves = ref<number[]>([]);
const editStatus = ref("");
const editBusy = ref(false);
const waterMetersInput = ref<number | null>(null);

const selectedBrush = computed(
  () => brushLists.value.find((b) => b.instance === selectedBrushInstance.value) ?? null,
);

async function loadBrushLists() {
  const packageId = selectedPackageId.value;
  if (packageId === null || !selectedGroup.value) return;
  try {
    brushLists.value = await invoke<BrushList[]>("map_panel_list_brushes", {
      packagePath: packagePathOf(packageId),
      group: selectedGroup.value,
    });
    selectedBrushInstance.value = "";
    placementMode.value = false;
    pendingAdds.value = [];
    pendingRemoves.value = [];
  } catch (e) {
    editStatus.value = String(e);
  }
}

function selectBrush(instance: string) {
  selectedBrushInstance.value = instance;
  pendingAdds.value = [];
  pendingRemoves.value = [];
  placementMode.value = false;
}

function toggleRemove(index: number) {
  const at = pendingRemoves.value.indexOf(index);
  if (at >= 0) pendingRemoves.value.splice(at, 1);
  else pendingRemoves.value.push(index);
}

function onMapClick(world: { x: number; y: number }) {
  if (!placementMode.value || !selectedBrushInstance.value) return;
  pendingAdds.value.push([world.x, world.y]);
}

async function saveBrushes() {
  const packageId = selectedPackageId.value;
  const brush = selectedBrush.value;
  if (!packageId || !brush) return;
  if (!pendingAdds.value.length && !pendingRemoves.value.length) return;
  const target = await saveFileDialog({
    title: t("studio.map.brushSave"),
    defaultPath: `${brush.name}.overlay.package`,
    filters: [{ name: "DBPF", extensions: ["package"] }],
  });
  if (!target) return;
  editBusy.value = true;
  try {
    const result = await invoke<OverlayWriteResult>("map_panel_edit_brush_stamps", {
      packagePath: packagePathOf(packageId),
      group: selectedGroup.value,
      brushInstance: brush.instance,
      add: pendingAdds.value,
      remove: pendingRemoves.value,
      strength: null,
      outPath: target,
    });
    editStatus.value = t("studio.map.savedTo", {
      count: result.entryCount,
      path: result.outPath,
    });
    pendingAdds.value = [];
    pendingRemoves.value = [];
    placementMode.value = false;
    await loadBrushLists();
  } catch (e) {
    editStatus.value = String(e);
  } finally {
    editBusy.value = false;
  }
}

async function saveWater() {
  const packageId = selectedPackageId.value;
  const meters = waterMetersInput.value;
  if (!packageId || meters === null || Number.isNaN(meters)) return;
  const target = await saveFileDialog({
    title: t("studio.map.waterSave"),
    defaultPath: "water-level.overlay.package",
    filters: [{ name: "DBPF", extensions: ["package"] }],
  });
  if (!target) return;
  editBusy.value = true;
  try {
    const result = await invoke<OverlayWriteResult>("map_panel_set_water_level", {
      packagePath: packagePathOf(packageId),
      group: selectedGroup.value,
      meters,
      outPath: target,
    });
    editStatus.value = t("studio.map.savedTo", {
      count: result.entryCount,
      path: result.outPath,
    });
  } catch (e) {
    editStatus.value = String(e);
  } finally {
    editBusy.value = false;
  }
}

async function importHeightmap() {
  const packageId = selectedPackageId.value;
  if (!packageId || !selectedGroup.value) return;
  const picked = await openFileDialog({
    multiple: false,
    filters: [{ name: "PNG", extensions: ["png"] }],
  });
  if (!picked || typeof picked !== "string") return;
  const target = await saveFileDialog({
    title: t("studio.map.importHeightmap"),
    defaultPath: "heightmap.overlay.package",
    filters: [{ name: "DBPF", extensions: ["package"] }],
  });
  if (!target) return;
  editBusy.value = true;
  editStatus.value = "";
  try {
    const result = await invoke<OverlayWriteResult>("map_panel_write_heightmap", {
      packagePath: packagePathOf(packageId),
      group: selectedGroup.value,
      heightsPngPath: picked,
      outPath: target,
    });
    editStatus.value = t("studio.map.savedTo", {
      count: result.entryCount,
      path: result.outPath,
    });
  } catch (e) {
    editStatus.value = String(e);
  } finally {
    editBusy.value = false;
  }
}

// ── L1 窗口高保真渲染：放大后按视口取局部重渲染（防抖 + 竞态保护）──
const detail = ref<RegionRender | null>(null);
let detailToken = 0;
let detailTimer: number | null = null;

interface ViewportChange {
  zoom: number;
  worldRect: { x0: number; y0: number; x1: number; y1: number };
}

function onViewportChange(viewport: ViewportChange) {
  if (selectedPackageId.value === null || !render.value) return;
  if (viewport.zoom < 2.5) {
    detail.value = null;
    return;
  }
  if (detailTimer !== null) window.clearTimeout(detailTimer);
  detailTimer = window.setTimeout(() => void fetchDetail(viewport.worldRect), 250);
}

async function fetchDetail(rect: ViewportChange["worldRect"]) {
  const packageId = selectedPackageId.value;
  if (packageId === null || !render.value) return;
  // 裁剪到已渲染区域范围（米）
  const origin = render.value.originWorld ?? [-16384, -16384];
  const extent = render.value.width * render.value.metersPerPixel;
  const x0 = Math.max(rect.x0, origin[0]);
  const y0 = Math.max(rect.y0, origin[1]);
  const x1 = Math.min(rect.x1, origin[0] + extent);
  const y1 = Math.min(rect.y1, origin[1] + extent);
  if (x1 <= x0 || y1 <= y0) return;
  const token = ++detailToken;
  try {
    const result = await invoke<RegionRender>("map_panel_render_region_window", {
      packagePath: packagePathOf(packageId),
      group: selectedGroup.value,
      x0,
      y0,
      x1,
      y1,
    });
    if (token === detailToken) detail.value = result;
  } catch {
    if (token === detailToken) detail.value = null;
  }
}

onBeforeUnmount(() => {
  if (detailTimer !== null) window.clearTimeout(detailTimer);
});
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
          @click="selectedGroup = r.group"
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
    <p v-if="!openedPackages.length" class="notice" role="status">
      {{ t("studio.map.needPackage") }}
    </p>

    <!-- 主区：左预览卡片 + 右面板 -->
    <div class="workspace">
      <div class="viewer-card">
        <!-- 顶部工具条：横向 flex 右对齐（分段式图层切换 + outline 展开） -->
        <div class="card-toolbar">
          <div class="layer-seg" role="group" :aria-label="t('studio.map.layersTitle')">
            <button
              type="button"
              :class="{ active: activeResource === null }"
              :title="t('studio.map.layerOff')"
              @click="activeResource = null"
            >
              {{ t("studio.map.layerOff") }}
            </button>
            <button
              v-for="kind in resourceKinds"
              :key="kind"
              type="button"
              :class="{ active: activeResource === kind }"
              :title="resourceLabel(kind)"
              @click="selectResource(kind)"
            >
              {{ resourceLabel(kind) }}
            </button>
          </div>
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
          <MapViewer
            :render="render"
            :show-plots="showPlots"
            :visible-resources="visibleResourceKinds"
            :detail="detail"
            :placing="placementMode"
            @map-click="onMapClick"
            @viewport-change="onViewportChange"
          />
        </div>
      </div>

      <aside class="side-panel">
        <section class="side-section">
          <h3>
            {{ renderRegionName || t("studio.map.propertiesTitle") }}
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
          <h3>{{ t("studio.map.resourcesTitle") }}</h3>
          <ul v-if="brushes.length" class="brush-list">
            <li v-for="[name, stamps] in brushes" :key="name">
              {{ name }} · {{ stamps.length }}
            </li>
          </ul>
          <p v-else class="side-empty">{{ t("studio.map.emptySide") }}</p>
        </section>

        <!-- 编辑（P1-5/P1-6）：水位 / 画刷 stamp / 高度图导入，overlay 副本写回 -->
        <section class="side-section">
          <h3>{{ t("studio.map.editTitle") }}</h3>
          <template v-if="render">
            <div class="edit-row">
              <label class="edit-label" for="water-input">
                {{ t("studio.map.waterLabel") }}
              </label>
              <input
                id="water-input"
                v-model.number="waterMetersInput"
                class="edit-input"
                type="number"
                step="1"
              />
              <button
                type="button"
                class="edit-button"
                :disabled="editBusy || waterMetersInput === null"
                @click="saveWater"
              >
                {{ t("studio.map.waterSave") }}
              </button>
            </div>
            <div class="edit-row">
              <button
                type="button"
                class="run-button"
                :disabled="editBusy || !selectedGroup"
                @click="importHeightmap"
              >
                <FIcon name="Download" :size="14" />
                {{ t("studio.map.importHeightmap") }}
              </button>
            </div>
            <h4 class="edit-sub">{{ t("studio.map.brushEditorTitle") }}</h4>
            <p v-if="!brushLists.length" class="side-note">
              {{ t("studio.map.emptySide") }}
            </p>
            <template v-else>
              <button
                v-for="b in brushLists"
                :key="b.instance"
                type="button"
                class="layer-option"
                :class="{ active: selectedBrushInstance === b.instance }"
                @click="selectBrush(b.instance)"
              >
                <FIcon
                  :name="selectedBrushInstance === b.instance ? 'Check' : 'Square'"
                  :size="13"
                  aria-label=""
                />
                {{ b.name }} · {{ b.stamps.length }}
              </button>
              <template v-if="selectedBrush">
                <label class="layer-toggle add-mode">
                  <FCheckbox v-model="placementMode" />
                  {{ t("studio.map.brushAddMode") }}
                </label>
                <p v-if="placementMode" class="side-note">
                  {{ t("studio.map.brushAddHint") }}
                </p>
                <ul class="stamp-list">
                  <li
                    v-for="(stamp, i) in selectedBrush.stamps"
                    :key="`s${i}`"
                    :class="{ removed: pendingRemoves.includes(i) }"
                  >
                    <span class="mono"
                      >{{ stamp[0].toFixed(0) }}, {{ stamp[1].toFixed(0) }}</span
                    >
                    <button type="button" class="stamp-btn" @click="toggleRemove(i)">
                      {{ t("studio.map.brushRemove") }}
                    </button>
                  </li>
                  <li v-for="(stamp, i) in pendingAdds" :key="`a${i}`" class="pending">
                    <span class="mono">{{ stamp[0] }}, {{ stamp[1] }}</span>
                    <button
                      type="button"
                      class="stamp-btn"
                      @click="pendingAdds.splice(i, 1)"
                    >
                      {{ t("studio.map.stampUndo") }}
                    </button>
                  </li>
                </ul>
                <button
                  type="button"
                  class="run-button edit-save"
                  :disabled="
                    editBusy || (!pendingAdds.length && !pendingRemoves.length)
                  "
                  @click="saveBrushes"
                >
                  {{ t("studio.map.brushSave") }}
                </button>
              </template>
              <p v-else class="side-note">{{ t("studio.map.brushNone") }}</p>
            </template>
            <p v-if="editStatus" class="side-note mono">{{ editStatus }}</p>
          </template>
          <p v-else class="side-empty">{{ t("studio.map.emptySide") }}</p>
        </section>

        <section class="side-section">
          <h3>{{ t("studio.map.layersTitle") }}</h3>
          <label class="layer-toggle">
            <FCheckbox v-model="showPlots" />
            {{ t("studio.map.layerPlots") }}
          </label>
          <button
            v-for="kind in resourceKinds"
            :key="kind"
            type="button"
            class="layer-option"
            :class="{ active: activeResource === kind }"
            @click="selectResource(kind)"
          >
            <FIcon
              :name="activeResource === kind ? 'Check' : 'Square'"
              :size="13"
              aria-label=""
            />
            {{ resourceLabel(kind) }}
          </button>
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
          <div class="layer-seg" role="group" :aria-label="t('studio.map.layersTitle')">
            <button
              type="button"
              :class="{ active: activeResource === null }"
              :title="t('studio.map.layerOff')"
              @click="activeResource = null"
            >
              {{ t("studio.map.layerOff") }}
            </button>
            <button
              v-for="kind in resourceKinds"
              :key="kind"
              type="button"
              :class="{ active: activeResource === kind }"
              :title="resourceLabel(kind)"
              @click="selectResource(kind)"
            >
              {{ resourceLabel(kind) }}
            </button>
          </div>
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
          <MapViewer
            v-if="sheetOpen"
            :render="render"
            :show-plots="showPlots"
            :visible-resources="visibleResourceKinds"
            :detail="detail"
            :placing="placementMode"
            @map-click="onMapClick"
            @viewport-change="onViewportChange"
          />
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
/* ── 编辑器（P1-5/P1-6）：水位 / 画刷 / 高度图导入 ── */
.edit-row {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin: 0.25rem 0;
}
.edit-label {
  color: var(--muted-foreground);
  font-size: 0.75rem;
}
.edit-input {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font-size: 0.75rem;
  padding: 4px 8px;
  width: 96px;
}
.edit-input:focus {
  outline: 1px solid var(--border-strong, var(--border));
}
.edit-button {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  font-size: 0.75rem;
  padding: 4px 10px;
}
.edit-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.edit-button:hover:not(:disabled) {
  border-color: var(--border-strong, var(--border));
}
.edit-sub {
  font-size: 0.8125rem;
  margin: 0.75rem 0 0.25rem;
}
.add-mode {
  margin-top: 0.5rem;
}
.stamp-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  list-style: none;
  margin: 0.5rem 0;
  max-height: 200px;
  overflow-y: auto;
  padding: 0;
}
.stamp-list li {
  align-items: center;
  display: flex;
  font-size: 0.7rem;
  gap: 0.5rem;
  justify-content: space-between;
}
.stamp-list li.removed {
  opacity: 0.45;
  text-decoration: line-through;
}
.stamp-list li.pending {
  color: var(--primary, var(--foreground));
}
.stamp-btn {
  background: transparent;
  border: none;
  color: var(--muted-foreground);
  cursor: pointer;
  font-size: 0.7rem;
  padding: 2px 4px;
}
.stamp-btn:hover {
  color: var(--foreground);
}
.edit-save {
  margin-top: 0.5rem;
  width: 100%;
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
/* 分段式切换（对齐 property editor 的默认/精细渲染切换） */
.layer-seg {
  display: inline-flex;
  flex-wrap: wrap;
}
.layer-seg button {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 6px;
  min-height: 24px;
  padding: 2px 9px;
}
.layer-seg button:first-child {
  border-end-end-radius: 0;
  border-start-end-radius: 0;
}
.layer-seg button:last-child {
  border-end-start-radius: 0;
  border-start-start-radius: 0;
  margin-inline-start: -1px;
}
.layer-seg button + button {
  margin-inline-start: -1px;
}
.layer-seg button.active {
  color: var(--foreground);
  opacity: 0.95;
}
.layer-seg button:hover {
  color: var(--foreground);
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
  flex: 1;
  min-height: 0;
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
.res-dot {
  border-radius: 50%;
  display: inline-block;
  flex-shrink: 0;
  height: 9px;
  width: 9px;
}
.layer-option {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font-size: 0.75rem;
  gap: 0.5rem;
  padding: 0.3rem 0.4rem;
  text-align: start;
  width: 100%;
}
.layer-option:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.layer-option.active {
  color: var(--foreground);
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
