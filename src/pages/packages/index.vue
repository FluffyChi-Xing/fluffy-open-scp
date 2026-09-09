<script setup lang="ts">
import { computed, onMounted, shallowRef } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import { isTauri, tauriApi } from "@/api";
import { useGamePackages } from "@/composables/useGamePackages";
import { useTabScroller } from "@/composables/useTabScroller";
import GameFolderTree from "./components/GameFolderTree.vue";
import HexPreview from "./components/preview/HexPreview.vue";
import ResourcePreview from "./components/preview/ResourcePreview.vue";
import {
  Resizable,
  ResizableHandle,
  ResizablePanel,
} from "@/components/ui/resizable";
import { resourceIconUrl, resourceKind } from "@/lib/resource-types";

const { t } = useI18n();
const explorer = useGamePackages();
const {
  root,
  folders,
  opened,
  activePackage,
  activePackageId,
  activePage,
  category,
  currentPage,
  totalPages,
  canPrev,
  canNext,
  selected,
  preview,
  previewLoading,
  previewError,
  loadingFolders,
  demo,
} = storeToRefs(explorer);
const importMode = shallowRef("folder");

// 进入页面即按持久化的游戏目录设置初始化目录树（store 内部有幂等保护）
onMounted(() => {
  void explorer.initFromSettings();
});
const detailMode = shallowRef<"hex" | "preview">("hex");
const copied = shallowRef(false);
let copiedTimer: number | undefined;

const typeTabs = computed(() => {
  const counts = activePage.value?.typeCounts ?? [];
  return [
    {
      key: "all",
      label: t("package.all"),
      count: activePage.value?.total ?? 0,
    },
    ...counts.map((entry) => ({
      key: String(entry.typeId),
      label: entry.name,
      count: entry.count,
    })),
  ];
});
const visibleResources = computed(() => activePage.value?.items ?? []);
const {
  scroller: tabsScroller,
  canStart: canScrollStart,
  canEnd: canScrollEnd,
  scroll: scrollTabs,
  update: updateTabScroll,
} = useTabScroller(typeTabs);
const {
  scroller: packageScroller,
  canStart: packageCanStart,
  canEnd: packageCanEnd,
  scroll: scrollPackageTabs,
  update: updatePackageTabScroll,
} = useTabScroller(opened);
function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(kb < 10 ? 1 : 0)} KB`;
  const mb = kb / 1024;
  if (mb < 1024) return `${mb.toFixed(mb < 10 ? 1 : 0)} MB`;
  return `${(mb / 1024).toFixed(1)} GB`;
}
async function copyTgi() {
  if (!selected.value) return;
  try {
    await navigator.clipboard.writeText(tgiLabel(selected.value.tgi));
    copied.value = true;
    window.clearTimeout(copiedTimer);
    copiedTimer = window.setTimeout(() => (copied.value = false), 1500);
  } catch {
    // 剪贴板不可用时静默失败
  }
}
function selectImportMode(event: Event) {
  importMode.value = (event.target as HTMLSelectElement).value;
}
async function importFromSelection() {
  if (importMode.value === "folder") {
    const path = isTauri()
      ? await tauriApi.workspace.pickDirectory("选择 SimCity 数据目录")
      : "D:/ea-games/SimCity";
    if (path) {
      root.value = path;
      await explorer.loadFolders(path);
    }
  } else if (isTauri()) {
    const status = await tauriApi.settings.get();
    if (status.gameDataPath) {
      root.value = status.gameDataPath;
      await explorer.loadFolders(status.gameDataPath);
    }
  } else {
    await explorer.loadFolders("D:/ea-games/SimCity");
  }
}
function tgiLabel(tgi: { typeId: number; group: number; instance: number }) {
  return [tgi.typeId, tgi.group, tgi.instance]
    .map((value) => value.toString(16).padStart(8, "0"))
    .join(":");
}
</script>

<template>
  <section class="packages-page">
    <header class="page-heading">
      <div>
        <p class="eyebrow">{{ $t("package.eyebrow") }}</p>
        <FTypography :header="1" spacing="none">{{
          $t("package.title")
        }}</FTypography
        ><FTypography paragraphy type="secondary">{{
          $t("package.ideDescription")
        }}</FTypography>
      </div>
    </header>
    <div class="explorer-toolbar">
      <div class="toolbar-title">
        <FIcon name="FolderOpen" :size="17" aria-label="" />
        <div>
          <strong>{{ root }}</strong
          ><small>{{
            demo ? $t("package.mockSource") : $t("package.liveSource")
          }}</small>
        </div>
      </div>
      <div class="toolbar-actions">
        <label for="import-mode" class="sr-only">{{
          $t("package.importMode")
        }}</label
        ><select
          id="import-mode"
          :value="importMode"
          @change="selectImportMode"
        >
          <option value="folder">{{ $t("package.chooseFolder") }}</option>
          <option value="default">
            {{ $t("package.useDefaultFolder") }}
          </option></select
        ><button
          class="toolbar-button"
          type="button"
          @click="importFromSelection"
        >
          <FIcon name="FolderOpen" :size="15" aria-label="" />{{
            $t("package.import")
          }}</button
        ><button
          class="toolbar-button"
          type="button"
          :disabled="loadingFolders"
          @click="explorer.loadFolders()"
        >
          <FIcon name="RefreshCw" :size="15" aria-label="" />{{
            $t("common.refresh")
          }}
        </button>
      </div>
    </div>
    <Resizable
      class="ide-shell"
      direction="horizontal"
      auto-save-id="openscp:package-layout:v1"
    >
      <ResizablePanel id="package-navigation" :default-size="23" :min-size="17">
        <GameFolderTree
          :folders="folders"
          :loading="loadingFolders"
          @open="explorer.openFile"
        />
      </ResizablePanel>
      <ResizableHandle
        orientation="horizontal"
        :label="$t('package.resizeNavigation')"
      />
      <ResizablePanel id="package-workspace" :default-size="52" :min-size="34">
        <section class="work-column">
          <div
            class="package-tabs"
            role="tablist"
            :aria-label="$t('package.openPackages')"
          >
            <p v-if="!opened.length" class="tabs-hint">
              {{ $t("package.emptyWorkspaceDescription") }}
            </p>
            <template v-else>
              <button
                class="tab-scroll-button"
                type="button"
                :disabled="!packageCanStart"
                :aria-label="$t('package.prevPage')"
                @click="scrollPackageTabs(-1)"
              >
                <FIcon name="ChevronLeft" :size="14" aria-label="" />
              </button>
              <div
                ref="packageScroller"
                class="package-tabs-scroll"
                @scroll.passive="updatePackageTabScroll"
              >
                <button
                  v-for="item in opened"
                  :key="item.package.packageId"
                  class="package-tab"
                  :class="{
                    active: item.package.packageId === activePackageId,
                  }"
                  type="button"
                  role="tab"
                  :aria-selected="item.package.packageId === activePackageId"
                  @click="explorer.choosePackage(item.package.packageId)"
                >
                  <FIcon name="Package" :size="14" aria-label="" /><span>{{
                    item.package.path.split(/[\\/]/).pop()
                  }}</span
                  ><span
                    class="tab-close"
                    @click.stop="explorer.closePackage(item.package.packageId)"
                    >×</span
                  >
                </button>
              </div>
              <button
                class="tab-scroll-button"
                type="button"
                :disabled="!packageCanEnd"
                :aria-label="$t('package.nextPage')"
                @click="scrollPackageTabs(1)"
              >
                <FIcon name="ChevronRight" :size="14" aria-label="" />
              </button>
            </template>
          </div>
          <template v-if="activePackage"
            ><div class="work-header">
              <div>
                <FTypography :header="3" spacing="none">{{
                  activePackage.package.path.split(/[\\/]/).pop()
                }}</FTypography
                ><FTypography paragraphy type="secondary" spacing="none"
                  >{{ activePackage.package.entryCount.toLocaleString() }}
                  {{ $t("package.resources") }} ·
                  {{ activePackage.package.kind }}</FTypography
                >
              </div>
              <span class="work-status"
                ><span aria-hidden="true" />{{ $t("common.ready") }}</span
              >
            </div>
            <nav
              class="category-tabs"
              role="tablist"
              :aria-label="$t('package.resourceCategories')"
            >
              <button
                class="tab-scroll-button"
                type="button"
                :disabled="!canScrollStart"
                :aria-label="$t('package.prevPage')"
                @click="scrollTabs(-1)"
              >
                <FIcon name="ChevronLeft" :size="14" aria-label="" />
              </button>
              <div
                ref="tabsScroller"
                class="category-tabs-scroll"
                @scroll.passive="updateTabScroll"
              >
                <button
                  v-for="item in typeTabs"
                  :key="item.key"
                  class="category-tab"
                  :class="{ active: category === item.key }"
                  type="button"
                  role="tab"
                  :aria-selected="category === item.key"
                  @click="explorer.chooseCategory(item.key)"
                >
                  <span>{{ item.label }}</span>
                  <span>{{ item.count }}</span>
                </button>
              </div>
              <button
                class="tab-scroll-button"
                type="button"
                :disabled="!canScrollEnd"
                :aria-label="$t('package.nextPage')"
                @click="scrollTabs(1)"
              >
                <FIcon name="ChevronRight" :size="14" aria-label="" />
              </button>
            </nav>
            <div class="resource-table-wrap">
              <table class="resource-table">
                <thead>
                  <tr>
                    <th>{{ $t("package.name") }}</th>
                    <th>{{ $t("package.type") }}</th>
                    <th>{{ $t("package.storage") }}</th>
                    <th>{{ $t("package.compression") }}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="resource in visibleResources"
                    :key="tgiLabel(resource.tgi)"
                    :class="{ selected: selected === resource }"
                    @click="explorer.selectResource(resource)"
                  >
                    <td>
                      <img
                        v-if="
                          resourceIconUrl(resourceKind(resource.tgi.typeId))
                        "
                        class="resource-icon"
                        :src="
                          resourceIconUrl(resourceKind(resource.tgi.typeId))
                        "
                        alt=""
                      /><span
                        class="resource-name"
                        :title="tgiLabel(resource.tgi)"
                        >{{ explorer.resourceLabel(resource) }}</span
                      >
                    </td>
                    <td class="resource-type">
                      {{ explorer.typeNameOf(resource) }}
                    </td>
                    <td>{{ formatSize(resource.decompressedSize) }}</td>
                    <td>
                      <span class="compression-badge">{{
                        resource.compressed
                          ? $t("package.compressed")
                          : $t("package.stored")
                      }}</span>
                    </td>
                  </tr>
                </tbody>
              </table>
              <p v-if="!visibleResources.length" class="empty-hint">
                {{ $t("package.noCategoryResources") }}
              </p>
              <div
                v-if="activePage && activePage.total > 0"
                class="pagination-bar"
              >
                <span class="pagination-summary">{{
                  $t("package.pageSummary", {
                    page: currentPage,
                    total: activePage.total,
                  })
                }}</span>
                <div class="pagination-actions">
                  <button
                    class="pagination-button"
                    type="button"
                    :disabled="!canPrev"
                    @click="explorer.prevPage()"
                  >
                    {{ $t("package.prevPage") }}
                  </button>
                  <span class="pagination-page"
                    >{{ currentPage }} / {{ totalPages }}</span
                  >
                  <button
                    class="pagination-button"
                    type="button"
                    :disabled="!canNext"
                    @click="explorer.nextPage()"
                  >
                    {{ $t("package.nextPage") }}
                  </button>
                </div>
              </div>
            </div></template
          >
          <div v-else class="work-empty">
            <FTypography :header="3">{{
              $t("package.selectPackageTitle")
            }}</FTypography
            ><FTypography paragraphy type="secondary">{{
              $t("package.selectPackageDescription")
            }}</FTypography>
          </div>
        </section>
      </ResizablePanel>
      <ResizableHandle
        orientation="horizontal"
        :label="$t('package.resizePreview')"
      />
      <ResizablePanel id="package-preview" :default-size="25" :min-size="20">
        <aside class="detail-column">
          <template v-if="selected"
            ><div class="detail-heading">
              <div>
                <FTypography :header="4" spacing="none">{{
                  explorer.resourceLabel(selected)
                }}</FTypography
                ><div class="detail-tgi">
                  <code>{{ tgiLabel(selected.tgi) }}</code>
                  <button class="copy-button" type="button" @click="copyTgi">
                    <FIcon
                      :name="copied ? 'Check' : 'Copy'"
                      :size="13"
                      aria-label=""
                    />{{
                      copied ? $t("package.copied") : $t("package.copyTgi")
                    }}
                  </button>
                </div>
              </div>
            </div>
            <div class="detail-tabs" role="tablist">
              <button
                class="detail-tab"
                :class="{ active: detailMode === 'hex' }"
                type="button"
                role="tab"
                :aria-selected="detailMode === 'hex'"
                @click="detailMode = 'hex'"
              >
                {{ $t("package.hex") }}</button
              ><button
                class="detail-tab"
                :class="{ active: detailMode === 'preview' }"
                type="button"
                role="tab"
                :aria-selected="detailMode === 'preview'"
                @click="detailMode = 'preview'"
              >
                {{ $t("package.preview") }}
              </button>
            </div>
            <div v-if="detailMode === 'hex'" class="detail-content">
              <div v-if="previewLoading" class="detail-loading">
                <FSkeleton height="18px" width="45%" rounded /><FSkeleton
                  height="12px"
                  width="80%"
                /><FSkeleton height="12px" width="65%" />
              </div>
              <p v-else-if="previewError" class="detail-error" role="alert">
                {{ previewError }}
              </p>
              <HexPreview v-else-if="preview" :preview="preview" />
              <p v-else class="detail-hint">{{ $t("package.bytesReady") }}</p>
            </div>
            <ResourcePreview
              v-else
              :preview="preview"
              :type-name="selected ? explorer.typeNameOf(selected) : ''"
              :loading="previewLoading"
              :error="previewError"
            />
          </template>
          <p v-else class="empty-hint">
            {{ $t("package.emptyDetailDescription") }}
          </p>
        </aside>
      </ResizablePanel>
    </Resizable>
  </section>
</template>

<style scoped>
.packages-page {
  display: grid;
  gap: 18px;
  min-height: 0;
}
.page-heading {
  align-items: flex-start;
  display: flex;
  justify-content: space-between;
}
.eyebrow {
  color: var(--primary);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.08em;
  margin: 0 0 10px;
  text-transform: uppercase;
}
.explorer-toolbar {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  gap: 16px;
  justify-content: space-between;
  padding: 10px 12px;
}
.toolbar-title {
  align-items: center;
  display: flex;
  gap: 9px;
  min-width: 0;
}
.toolbar-title > svg {
  color: var(--primary);
  flex: none;
}
.toolbar-title div {
  display: grid;
  gap: 2px;
  min-width: 0;
}
.toolbar-title strong {
  font:
    12px ui-monospace,
    SFMono-Regular,
    Menlo,
    Consolas,
    monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.toolbar-title small {
  color: var(--muted-foreground);
  font-size: 11px;
}
.toolbar-actions {
  display: flex;
  gap: 7px;
}
.toolbar-actions select,
.toolbar-button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  min-height: 34px;
  padding: 0 10px;
}
.toolbar-button {
  align-items: center;
  cursor: pointer;
  display: inline-flex;
  gap: 6px;
  font-weight: 700;
}
.toolbar-button:disabled {
  cursor: wait;
  opacity: 0.5;
}
.ide-shell {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  height: min(720px, calc(100vh - 300px));
  max-height: 720px;
  min-height: 520px;
  overflow: hidden;
  width: 100%;
}
.tree-column,
.work-column,
.detail-column {
  background: var(--surface);
  min-width: 0;
  min-height: 0;
  height: 100%;
}
.work-column {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.detail-column {
  overflow: auto;
  padding: 14px;
}
.tabs-hint {
  align-items: center;
  color: var(--subtle-foreground);
  display: flex;
  flex: 1;
  font-size: 11px;
  justify-content: center;
  margin: 0;
}
.empty-hint {
  color: var(--subtle-foreground);
  font-size: 12px;
  margin: 0;
  padding: 26px 12px;
  text-align: center;
}
.package-tabs {
  align-items: stretch;
  background: var(--surface-elevated);
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  min-height: 40px;
}
.package-tabs-scroll {
  display: flex;
  flex: 1;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}
.package-tabs-scroll::-webkit-scrollbar {
  display: none;
}
.package-tab {
  align-items: center;
  background: transparent;
  border: 0;
  border-inline-end: 1px solid var(--border);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 7px;
  max-width: 210px;
  padding: 0 11px;
  transition:
    background-color 140ms ease,
    color 140ms ease;
}
.package-tab:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.package-tab.active {
  background: var(--accent);
  box-shadow: inset 0 -2px var(--primary);
  color: var(--foreground);
  font-weight: 700;
}
.package-tab span:nth-child(2) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tab-close {
  border-radius: 4px;
  font-size: 16px;
  line-height: 1;
  padding: 2px;
}
.tab-close:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.work-header {
  align-items: flex-start;
  display: flex;
  flex: none;
  justify-content: space-between;
  min-width: 0;
  padding: 16px 18px 12px;
}
.work-header > div:first-child {
  min-width: 0;
}
.work-status {
  align-items: center;
  color: var(--success);
  display: flex;
  font-size: 11px;
  gap: 6px;
}
.work-status span {
  background: currentColor;
  border-radius: 50%;
  height: 6px;
  width: 6px;
}
.category-tabs {
  align-items: stretch;
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  gap: 2px;
  padding: 0 8px;
}
.category-tabs-scroll {
  display: flex;
  flex: 1;
  gap: 3px;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}
.category-tabs-scroll::-webkit-scrollbar {
  display: none;
}
.tab-scroll-button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  flex: none;
  justify-content: center;
  padding: 0 4px;
}
.tab-scroll-button:hover:not(:disabled) {
  color: var(--foreground);
}
.tab-scroll-button:disabled {
  color: var(--subtle-foreground);
  cursor: default;
  opacity: 0.45;
}
.category-tab {
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  padding: 10px 8px;
  white-space: nowrap;
}
.category-tab:hover,
.category-tab.active {
  color: var(--foreground);
}
.category-tab.active {
  border-bottom-color: var(--primary);
  font-weight: 700;
}
.category-tab span:last-child {
  background: var(--surface-hover);
  border-radius: 999px;
  font-size: 10px;
  margin-inline-start: 3px;
  padding: 2px 5px;
}
.category-tab.active span:last-child {
  background: var(--primary);
  color: var(--primary-foreground);
}
.resource-table-wrap {
  flex: 1 1 0;
  min-height: 0;
  overflow: auto;
  padding: 0 14px 14px;
}
.resource-table {
  border-collapse: collapse;
  font-size: 12px;
  min-width: 100%;
  text-align: start;
}
.resource-table th {
  color: var(--muted-foreground);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.06em;
  padding: 11px 8px;
  text-align: start;
  text-transform: uppercase;
}
.resource-table td {
  border-top: 1px solid var(--border);
  padding: 11px 8px;
}
.resource-table tbody tr {
  cursor: pointer;
}
.resource-table tbody tr:hover {
  background: var(--accent);
}
.resource-table tbody tr.selected {
  background: var(--accent);
  box-shadow: inset 3px 0 0 var(--primary);
}
.resource-table tbody tr.selected .resource-name {
  color: var(--foreground);
  font-weight: 700;
}
.resource-table td:first-child {
  align-items: center;
  display: flex;
  gap: 7px;
}
.resource-table td:first-child svg {
  color: var(--primary);
}
.resource-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.resource-icon {
  flex: none;
  height: 15px;
  object-fit: contain;
  width: 15px;
}
.category-tab {
  align-items: center;
  display: inline-flex;
  gap: 4px;
}
.resource-table code,
.detail-heading code {
  font:
    11px ui-monospace,
    SFMono-Regular,
    Menlo,
    Consolas,
    monospace;
}
.resource-type {
  color: var(--muted-foreground);
  white-space: nowrap;
}
.pagination-bar {
  align-items: center;
  border-top: 1px solid var(--border);
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  justify-content: space-between;
  margin-top: 10px;
  padding: 10px 0 2px;
}
.pagination-summary {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.pagination-actions {
  align-items: center;
  display: flex;
  gap: 8px;
}
.pagination-page {
  color: var(--muted-foreground);
  font:
    11px ui-monospace,
    SFMono-Regular,
    Menlo,
    Consolas,
    monospace;
}
.pagination-button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  min-height: 28px;
  padding: 0 10px;
}
.pagination-button:disabled {
  cursor: default;
  opacity: 0.45;
}
.detail-tgi {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 4px;
}
.copy-button {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 10px;
  gap: 4px;
  min-height: 24px;
  padding: 0 8px;
}
.copy-button:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.compression-badge {
  color: var(--muted-foreground);
  font-size: 10px;
}
.detail-heading {
  align-items: flex-start;
  display: flex;
  justify-content: space-between;
}
.detail-tabs {
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 14px;
  margin-top: 20px;
}
.detail-tab {
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--muted-foreground);
  font: inherit;
  font-size: 11px;
  padding: 9px 2px;
}
.detail-tab.active {
  border-bottom-color: var(--primary);
  color: var(--foreground);
  font-weight: 700;
}
.hex-view {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  font:
    11px/1.7 ui-monospace,
    SFMono-Regular,
    Menlo,
    Consolas,
    monospace;
  margin-top: 14px;
  min-height: 180px;
  overflow: auto;
  padding: 12px;
  white-space: pre-wrap;
}
.work-empty {
  align-items: center;
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 9px;
  justify-content: center;
  padding: 20px;
  text-align: center;
}
.sr-only {
  height: 1px;
  margin: -1px;
  overflow: hidden;
  position: absolute;
  width: 1px;
}
@media (max-width: 700px) {
  .explorer-toolbar {
    align-items: stretch;
    flex-direction: column;
  }
  .toolbar-actions {
    width: 100%;
  }
  .toolbar-actions > * {
    flex: 1;
  }
  .page-heading {
    gap: 12px;
  }
}
</style>
