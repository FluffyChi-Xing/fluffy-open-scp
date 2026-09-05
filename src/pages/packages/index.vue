<script setup lang="ts">
import { computed, shallowRef } from "vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import { useGamePackages } from "@/composables/useGamePackages";
import GameFolderTree from "./components/GameFolderTree.vue";
import PackageFileList from "./components/PackageFileList.vue";

const explorer = useGamePackages();
const {
  root,
  folders,
  files,
  opened,
  activePackage,
  activePackageId,
  activePage,
  selected,
  loadingFolders,
  loadingFiles,
  demo,
  selectedFolder,
} = explorer;
const importMode = shallowRef("folder");
const category = shallowRef("all");
const categories = computed(() => [
  {
    key: "all",
    label: "package.all",
    count: activePage.value?.total ?? 0,
  },
  { key: "rw4", label: "package.rw4", count: categoryCount("rw4") },
  { key: "raster", label: "package.raster", count: categoryCount("raster") },
  {
    key: "property",
    label: "package.property",
    count: categoryCount("property"),
  },
  { key: "text", label: "package.text", count: categoryCount("text") },
  { key: "media", label: "package.media", count: categoryCount("media") },
]);
const visibleResources = computed(() => {
  const items = activePage.value?.items ?? [];
  if (category.value === "all") return items;
  return items.filter(
    (item) => resourceCategory(item.tgi.typeId) === category.value,
  );
});
function categoryCount(key: string) {
  return (activePage.value?.items ?? []).filter(
    (item) => resourceCategory(item.tgi.typeId) === key,
  ).length;
}
function resourceCategory(typeId: number) {
  if (typeId === 0x2f4e681b) return "rw4";
  if (typeId === 0x2f4e681c) return "raster";
  if (typeId === 0x00b1b104) return "property";
  if (typeId === 0x0d9e5710) return "media";
  return "text";
}
function selectImportMode(event: Event) {
  importMode.value = (event.target as HTMLSelectElement).value;
}
async function importFromSelection() {
  if (importMode.value === "folder" || importMode.value === "default")
    await explorer.loadFolders("D:/ea-games/SimCity");
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
      <div class="heading-mark" aria-hidden="true">
        <FIcon name="Package" :size="22" />
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
    <div class="ide-shell">
      <aside class="tree-column">
        <GameFolderTree
          :folders="folders"
          :selected-path="selectedFolder"
          :loading="loadingFolders"
          @select="explorer.selectFolder"
        /><PackageFileList
          :files="files"
          :selected-path="activePackage?.package.path ?? ''"
          :loading="loadingFiles"
          @select="explorer.openFile"
        />
      </aside>
      <section class="work-column">
        <div
          class="package-tabs"
          role="tablist"
          :aria-label="$t('package.openPackages')"
        >
          <FEmpty
            v-if="!opened.length"
            icon-name="Package"
            :title="$t('package.emptyWorkspaceTitle')"
            :desc="$t('package.emptyWorkspaceDescription')"
            variant="compact"
          /><template v-else
            ><button
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
            </button></template
          >
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
              v-for="item in categories"
              :key="item.key"
              class="category-tab"
              :class="{ active: category === item.key }"
              type="button"
              role="tab"
              :aria-selected="category === item.key"
              @click="category = item.key"
            >
              {{ $t(item.label) }} <span>{{ item.count }}</span>
            </button>
          </nav>
          <div class="resource-table-wrap">
            <table class="resource-table">
              <thead>
                <tr>
                  <th>{{ $t("package.tgi") }}</th>
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
                    <FIcon name="FileText" :size="15" aria-label="" /><code>{{
                      tgiLabel(resource.tgi)
                    }}</code>
                  </td>
                  <td>{{ resource.decompressedSize.toLocaleString() }} B</td>
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
            <FEmpty
              v-if="!visibleResources.length"
              icon-name="Search"
              :title="$t('package.noCategoryResources')"
              :desc="$t('package.adjustCategory')"
              variant="compact"
            /></div
        ></template>
        <div v-else class="work-empty">
          <FIcon name="CircleDot" :size="24" aria-label="" /><FTypography
            :header="3"
            >{{ $t("package.selectPackageTitle") }}</FTypography
          ><FTypography paragraphy type="secondary">{{
            $t("package.selectPackageDescription")
          }}</FTypography
          ><FSkeleton width="70%" height="10px" rounded /><FSkeleton
            width="45%"
            height="10px"
            rounded
          />
        </div>
      </section>
      <aside class="detail-column">
        <template v-if="selected"
          ><div class="detail-heading">
            <div>
              <FTypography :header="4" spacing="none">{{
                $t("package.resourceDetail")
              }}</FTypography
              ><code>{{ tgiLabel(selected.tgi) }}</code>
            </div>
            <FIcon
              name="FileText"
              :size="19"
              color="var(--primary)"
              aria-label=""
            />
          </div>
          <div class="detail-tabs" role="tablist">
            <button
              class="detail-tab active"
              type="button"
              role="tab"
              aria-selected="true"
            >
              {{ $t("package.hex") }}</button
            ><button
              class="detail-tab"
              type="button"
              role="tab"
              aria-selected="false"
              disabled
            >
              {{ $t("package.preview") }}
            </button>
          </div>
          <pre class="hex-view">{{
            selected ? $t("package.bytesReady") : ""
          }}</pre></template
        ><FEmpty
          v-else
          icon-name="FileText"
          :title="$t('package.emptyDetailTitle')"
          :desc="$t('package.emptyDetailDescription')"
          variant="compact"
        />
      </aside>
    </div>
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
.heading-mark {
  align-items: center;
  background: var(--accent);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  color: var(--primary);
  display: flex;
  height: 52px;
  justify-content: center;
  width: 52px;
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
  display: grid;
  gap: 1px;
  grid-template-columns: minmax(210px, 240px) minmax(420px, 1fr) minmax(
      260px,
      340px
    );
  height: min(720px, calc(100vh - 300px));
  max-height: 720px;
  min-height: 520px;
  overflow: hidden;
}
.tree-column,
.work-column,
.detail-column {
  background: var(--surface);
  border: 1px solid var(--border);
  min-width: 0;
  overflow: auto;
  padding: 10px;
}
.tree-column {
  display: grid;
  align-content: start;
  gap: 10px;
}
.package-files {
  margin-top: 0;
}
.work-column {
  display: flex;
  flex-direction: column;
  padding: 0;
}
.package-tabs {
  align-items: stretch;
  background: var(--surface-elevated);
  border-bottom: 1px solid var(--border);
  display: flex;
  min-height: 48px;
  overflow-x: auto;
}
.package-tabs :deep(.f-empty) {
  min-width: 100%;
  padding: 12px;
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
  background: var(--surface);
  box-shadow: inset 0 -2px var(--primary);
  color: var(--foreground);
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
  justify-content: space-between;
  padding: 16px 18px 12px;
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
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 3px;
  overflow-x: auto;
  padding: 0 14px;
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
.category-tab span {
  background: var(--surface-hover);
  border-radius: 999px;
  font-size: 10px;
  margin-inline-start: 3px;
  padding: 2px 5px;
}
.resource-table-wrap {
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
.resource-table tbody tr:hover,
.resource-table tbody tr.selected {
  background: var(--accent);
}
.resource-table td:first-child {
  align-items: center;
  display: flex;
  gap: 7px;
}
.resource-table td:first-child svg {
  color: var(--primary);
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
.compression-badge {
  color: var(--muted-foreground);
  font-size: 10px;
}
.detail-column {
  padding: 18px;
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
.work-empty svg {
  color: var(--muted-foreground);
}
.work-empty .f-skeleton {
  margin-top: 7px;
}
.sr-only {
  height: 1px;
  margin: -1px;
  overflow: hidden;
  position: absolute;
  width: 1px;
}
@media (max-width: 1000px) {
  .ide-shell {
    grid-template-columns: 210px minmax(400px, 1fr);
  }
  .detail-column {
    display: none;
  }
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
  .ide-shell {
    grid-template-columns: 1fr;
    height: min(700px, calc(100vh - 360px));
    min-height: 480px;
  }
  .tree-column {
    max-height: 240px;
  }
  .work-column {
    min-height: 400px;
  }
  .page-heading {
    gap: 12px;
  }
  .heading-mark {
    height: 42px;
    width: 42px;
  }
}
</style>
