<script setup lang="ts">
import { onMounted } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import { isTauri } from "@/api";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { useLocaleEditorStore } from "@/stores/localeEditor";

const { t } = useI18n();
const editor = useLocaleEditorStore();
const gamePackages = useGamePackagesStore();
const {
  packageId,
  tables,
  loadingTables,
  activeTgi,
  activeTable,
  items,
  loadingItems,
  saving,
  filter,
  filteredItems,
  dirtyCount,
  hasEdits,
} = storeToRefs(editor);

onMounted(() => {
  // 已打开过 package 且未选过时自动选第一个（通常是刚在资源页打开的）
  if (packageId.value === null && gamePackages.opened.length) {
    void editor.loadTables(gamePackages.opened[0].package.packageId);
  }
});

function hex(value: number) {
  return (value >>> 0).toString(16).toUpperCase().padStart(8, "0");
}
</script>

<template>
  <section class="i18n-page">
    <RouterLink class="back-link" to="/studio">
      <FIcon name="ArrowLeft" :size="14" />
      {{ t("studio.backToStudio") }}
    </RouterLink>

    <header class="page-header">
      <span class="page-icon"><FIcon name="Globe" :size="20" /></span>
      <div>
        <FTypography :header="2" spacing="none">{{
          t("studio.panels.i18n.title")
        }}</FTypography>
        <p class="page-meta">{{ t("studio.panels.i18n.meta") }}</p>
      </div>
    </header>

    <p v-if="!isTauri()" class="notice" role="status">
      {{ t("studio.i18n.mockNotice") }}
    </p>
    <p v-else-if="!gamePackages.opened.length" class="notice" role="status">
      {{ t("studio.i18n.needPackage") }}
      <RouterLink to="/packages" class="inline-link">{{
        t("navigation.packages")
      }}</RouterLink>
    </p>

    <template v-if="isTauri() && gamePackages.opened.length">
      <section class="config-row">
        <label class="field">
          <span class="field-label">{{ t("studio.i18n.selectPackage") }}</span>
          <select
            class="field-input mono"
            :value="packageId ?? ''"
            @change="
              editor.loadTables(Number(($event.target as HTMLSelectElement).value))
            "
          >
            <option
              v-for="opened in gamePackages.opened"
              :key="opened.package.packageId"
              :value="opened.package.packageId"
            >
              {{ opened.package.path.split(/[\/]/).pop() }}
            </option>
          </select>
        </label>
        <span v-if="hasEdits" class="dirty-badge">
          {{ t("studio.i18n.dirtyCount", dirtyCount) }}
        </span>
      </section>

      <div class="editor-grid">
        <aside class="table-list" :aria-label="t('studio.i18n.tablesLabel')">
          <div v-if="loadingTables" class="skeleton-block">
            <FSkeleton v-for="index in 6" :key="index" height="44px" />
          </div>
          <p v-else-if="!tables.length" class="empty-hint">
            {{ t("studio.i18n.noTables") }}
          </p>
          <ul v-else class="tables">
            <li v-for="table in tables" :key="table.tableId">
              <button
                type="button"
                class="table-row"
                :class="{
                  active:
                    activeTgi &&
                    activeTgi.group === table.tgi.group &&
                    activeTgi.instance === table.tgi.instance,
                }"
                @click="editor.openTable(table)"
              >
                <span class="table-id mono">0x{{ hex(table.tableId) }}</span>
                <span class="table-count mono">{{ table.stringCount }}</span>
                <span class="table-sample" :title="table.sample.join(' / ')">{{
                  table.sample[0] ?? "—"
                }}</span>
              </button>
            </li>
          </ul>
        </aside>

        <div class="editor-main">
          <div v-if="loadingItems" class="skeleton-block">
            <FSkeleton v-for="index in 8" :key="index" height="40px" />
          </div>
          <div v-else-if="!activeTgi" class="empty-hint centered">
            {{ t("studio.i18n.pickTable") }}
          </div>
          <template v-else>
            <div class="editor-toolbar">
              <h2 class="section-title mono">
                0x{{ hex(activeTable?.tableId ?? activeTgi.instance) }}
                <span class="count-badge">{{ items.length }}</span>
              </h2>
              <input
                v-model="filter"
                type="search"
                class="field-input slim"
                :placeholder="t('studio.i18n.filterPlaceholder')"
              />
              <button
                type="button"
                class="ghost-button"
                :disabled="!hasEdits || saving"
                @click="editor.revertAll()"
              >
                <FIcon name="RotateCcw" :size="14" />
                {{ t("studio.i18n.revertAll") }}
              </button>
              <button
                type="button"
                class="run-button"
                :disabled="!hasEdits || saving"
                @click="editor.exportOverlay()"
              >
                <FIcon :name="saving ? 'LoaderCircle' : 'Save'" :size="14" />
                {{
                  saving
                    ? t("studio.i18n.saving")
                    : t("studio.i18n.exportOverlay")
                }}
              </button>
            </div>
            <div class="rows-scroll">
              <table class="string-table">
                <thead>
                  <tr>
                    <th class="col-key">{{ t("studio.i18n.colKey") }}</th>
                    <th class="col-text">{{ t("studio.i18n.colText") }}</th>
                    <th class="col-actions" />
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="item in filteredItems"
                    :key="item.key"
                    :class="{ comment: item.id === null, dirty: editor.isDirty(item) }"
                  >
                    <td class="col-key mono">{{ item.key }}</td>
                    <td class="col-text">
                      <input
                        v-if="item.id !== null"
                        v-model="item.text"
                        class="text-input"
                        type="text"
                      />
                      <span v-else class="comment-text">{{ item.text }}</span>
                    </td>
                    <td class="col-actions">
                      <button
                        v-if="item.id !== null && editor.isDirty(item)"
                        type="button"
                        class="ghost-button slim"
                        :title="t('studio.i18n.revert')"
                        @click="editor.revert(item)"
                      >
                        <FIcon name="RotateCcw" :size="13" />
                      </button>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </template>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.i18n-page {
  padding-bottom: 3rem;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}
.back-link {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.75rem;
  color: var(--muted-foreground);
  text-decoration: none;
  width: fit-content;
}
.back-link:hover {
  color: var(--foreground);
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
  color: var(--accent);
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
.notice {
  margin: 0;
  padding: 0.7rem 1rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--muted-foreground);
  font-size: 0.8125rem;
}
.inline-link {
  color: var(--accent);
}
.config-row {
  display: flex;
  align-items: flex-end;
  gap: 0.875rem;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  flex: 1;
  max-width: 480px;
}
.field-label {
  font-size: 0.75rem;
  color: var(--muted-foreground);
}
.field-input {
  padding: 0.5rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--background);
  color: var(--foreground);
  font-size: 0.8125rem;
}
.field-input.slim {
  padding: 0.35rem 0.6rem;
  width: 200px;
}
.mono {
  font-family: var(--font-mono, ui-monospace, monospace);
}
.dirty-badge {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  padding: 0.25rem 0.6rem;
  border-radius: 999px;
  border: 1px solid color-mix(in oklab, var(--warning) 50%, transparent);
  color: var(--warning);
}
.editor-grid {
  display: grid;
  grid-template-columns: minmax(220px, 300px) 1fr;
  gap: 0.875rem;
  min-height: 480px;
}
.table-list {
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  overflow-y: auto;
  max-height: 640px;
}
.tables {
  list-style: none;
  margin: 0;
  padding: 0.25rem;
  display: grid;
  gap: 0.2rem;
}
.table-row {
  display: grid;
  grid-template-columns: auto auto 1fr;
  gap: 0.6rem;
  align-items: center;
  width: 100%;
  padding: 0.5rem 0.6rem;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--foreground);
  cursor: pointer;
  text-align: left;
  font-size: 0.75rem;
}
.table-row:hover {
  background: color-mix(in oklab, var(--accent) 7%, transparent);
}
.table-row.active {
  background: color-mix(in oklab, var(--accent) 14%, transparent);
}
.table-id {
  color: var(--muted-foreground);
}
.table-count {
  color: var(--subtle-foreground);
}
.table-sample {
  color: var(--subtle-foreground);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.editor-main {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  min-width: 0;
}
.editor-toolbar {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
}
.section-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin: 0;
  font-size: 0.875rem;
  font-weight: 600;
}
.count-badge {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  padding: 0.1rem 0.5rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--muted-foreground);
}
.editor-toolbar .run-button,
.editor-toolbar .ghost-button {
  margin-left: auto;
}
.editor-toolbar .ghost-button {
  margin-left: 0;
}
.run-button {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.45rem 0.9rem;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: var(--primary);
  color: var(--primary-foreground);
  font-size: 0.8125rem;
  cursor: pointer;
}
.run-button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.ghost-button {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.45rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--muted-foreground);
  font-size: 0.8125rem;
  cursor: pointer;
}
.ghost-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.ghost-button.slim {
  padding: 0.25rem 0.4rem;
}
.rows-scroll {
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  overflow: auto;
  max-height: 640px;
}
.string-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.8125rem;
}
.string-table th {
  position: sticky;
  top: 0;
  background: var(--surface);
  text-align: left;
  font-size: 0.6875rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--subtle-foreground);
  padding: 0.55rem 0.75rem;
  border-bottom: 1px solid var(--border);
}
.string-table td {
  padding: 0.3rem 0.75rem;
  border-bottom: 1px solid color-mix(in oklab, var(--border) 55%, transparent);
  vertical-align: middle;
}
.col-key {
  width: 130px;
  color: var(--muted-foreground);
  font-size: 0.75rem;
}
.col-actions {
  width: 44px;
  text-align: right;
}
.text-input {
  width: 100%;
  padding: 0.35rem 0.5rem;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--foreground);
  font-size: 0.8125rem;
}
.text-input:hover {
  border-color: var(--border);
}
.text-input:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: -1px;
  background: var(--background);
}
tr.dirty .text-input {
  border-color: color-mix(in oklab, var(--warning) 45%, transparent);
  background: color-mix(in oklab, var(--warning) 6%, transparent);
}
tr.comment .col-text,
.comment-text {
  color: var(--subtle-foreground);
  font-style: italic;
}
.skeleton-block {
  display: grid;
  gap: 0.4rem;
  padding: 0.5rem;
}
.empty-hint {
  margin: 0;
  padding: 1.25rem;
  font-size: 0.8125rem;
  color: var(--subtle-foreground);
}
.empty-hint.centered {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 240px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-lg);
}
@media (max-width: 900px) {
  .editor-grid {
    grid-template-columns: 1fr;
  }
}
</style>
