<script setup lang="ts">
/**
 * 菜单编辑面板：显示当前选中菜单级的条目预览（图标 + 名称 + 排序）。
 * 点击条目 → 打开单条编辑 Sheet（由宿主页控制）；
 * 末位虚线加号块 → 追加新条目。底部为落库/还原动作。
 */
import { computed } from "vue";
import { storeToRefs } from "pinia";
import FIcon from "@/components/extensions/FIcon.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import { toolIconName } from "@/lib/game-ui/workbench";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const store = useUiWorkbenchStore();
const { activeMenu, edits } = storeToRefs(store);

const editedCount = computed(() => Object.keys(edits.value).length);

function emitEdit(id: string): void {
  const entry = activeMenu.value?.entries.find((candidate) => candidate.tool.id === id);
  if (entry) store.editingEntry = entry;
}

function onExport(): void {
  const json = store.exportOverlay();
  void navigator.clipboard?.writeText(json).then(
    () => window.alert(t("studio.workbench.exportCopied")),
    () => window.alert(json),
  );
}

function onAdd(): void {
  if (!activeMenu.value) return;
  store.addItem(activeMenu.value.id, t("studio.workbench.newItem"), null);
  const additions = store.added[activeMenu.value.id] ?? [];
  const created = additions.at(-1);
  if (created && activeMenu.value) {
    store.editingEntry = { menuId: activeMenu.value.id, tool: created, isNew: true };
  }
}
</script>

<template>
  <aside class="menu-panel">
    <header class="panel-head">
      <div>
        <p class="panel-eyebrow">{{ t("studio.workbench.panelTitle") }}</p>
        <h2 class="panel-title">
          {{ activeMenu?.label ?? "—" }}
          <span v-if="activeMenu" class="panel-count">{{ activeMenu.entries.length }}</span>
        </h2>
      </div>
      <span v-if="editedCount" class="edit-badge">{{ editedCount }} Δ</span>
    </header>

    <FEmpty
      v-if="!activeMenu"
      class="empty-body"
      variant="compact"
      icon-name="MousePointerClick"
      :title="t('studio.workbench.panelHint')"
    />

    <div v-else class="item-list">
      <button
        v-for="entry in activeMenu.entries"
        :key="entry.tool.id"
        type="button"
        class="item-card"
        @click="emitEdit(entry.tool.id)"
      >
        <span class="item-thumb">
          <FIcon :name="toolIconName(entry.tool)" :size="22" aria-label="" />
        </span>
        <span class="item-body">
          <span class="item-label">
            {{ entry.tool.label }}
            <span v-if="entry.isNew" class="tag-new">{{ t("studio.workbench.newTag") }}</span>
            <span v-else-if="edits[entry.tool.id]" class="tag-edit">Δ</span>
          </span>
          <span class="item-meta">
            <span class="tabnum">pos {{ entry.tool.pos }}</span>
            <span class="dot">·</span>
            <span>{{ entry.tool.source === "locale" ? t("studio.workbench.srcLocale") : t("studio.workbench.srcUnresolved") }}</span>
          </span>
        </span>
        <FIcon class="item-chev" name="ChevronRight" :size="14" aria-label="" />
      </button>

      <!-- 菜单末位：新增 -->
      <button type="button" class="add-tile" @click="onAdd">
        <span class="add-plus" aria-hidden="true">＋</span>
        <span>{{ t("studio.workbench.addItem") }}</span>
      </button>
    </div>

    <footer class="panel-foot">
      <button type="button" class="foot-btn" :disabled="!editedCount" @click="onExport">
        {{ t("studio.workbench.export") }}
      </button>
      <button type="button" class="foot-btn danger" :disabled="!editedCount" @click="store.resetAll()">
        {{ t("studio.workbench.reset") }}
      </button>
    </footer>
  </aside>
</template>

<style scoped>
.menu-panel {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  display: flex;
  flex-direction: column;
  min-height: 0;
  width: 320px;
}
.panel-head {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  justify-content: space-between;
  padding: 12px 14px 10px;
}
.panel-eyebrow {
  color: var(--muted-foreground);
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.08em;
  margin: 0;
  text-transform: uppercase;
}
.panel-title {
  font-size: 16px;
  margin: 2px 0 0;
}
.panel-count {
  background: var(--surface-hover);
  border-radius: 999px;
  color: var(--muted-foreground);
  font-size: 11px;
  padding: 1px 8px;
  vertical-align: 2px;
}
.edit-badge {
  background: color-mix(in srgb, var(--primary) 14%, transparent);
  border-radius: 999px;
  color: var(--primary);
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
}
.item-list {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 6px;
  list-style: none;
  margin: 0;
  min-height: 0;
  overflow: auto;
  padding: 10px;
}
.empty-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  /* 撑高后 grid 行不再 stretch：图标+文字整组垂直居中，行距统一 1rem */
  align-content: center;
  gap: 1rem;
  padding: 24px 16px;
}
.empty-body :deep(.f-empty-title) {
  margin-top: 0;
}
.item-card {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  cursor: pointer;
  display: flex;
  gap: 10px;
  padding: 8px 10px;
  text-align: start;
  transition: background-color 120ms ease, border-color 120ms ease;
  width: 100%;
}
.item-card:hover {
  background: var(--surface-hover);
  border-color: var(--border-strong);
}
.item-thumb {
  align-items: center;
  background: var(--surface-hover);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: inline-flex;
  flex: none;
  height: 44px;
  justify-content: center;
  overflow: hidden;
  width: 44px;
}
.item-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.item-label {
  color: var(--foreground);
  font-size: 12.5px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tag-new {
  background: color-mix(in srgb, var(--success) 16%, transparent);
  border-radius: 999px;
  color: var(--success);
  font-size: 10px;
  margin-inline-start: 4px;
  padding: 0 6px;
  vertical-align: 1px;
}
.tag-edit {
  color: var(--primary);
  font-size: 10px;
  margin-inline-start: 4px;
}
.item-meta {
  color: var(--muted-foreground);
  display: flex;
  font-size: 10.5px;
  gap: 5px;
}
.dot {
  opacity: 0.5;
}
.item-chev {
  color: var(--muted-foreground);
  flex: none;
  margin-inline-start: auto;
}
.add-tile {
  align-items: center;
  background: transparent;
  border: 1.5px dashed var(--border-strong);
  border-radius: var(--radius-md);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font-size: 12px;
  gap: 8px;
  justify-content: center;
  padding: 12px;
  transition: border-color 120ms ease, color 120ms ease;
}
.add-tile:hover {
  border-color: var(--primary);
  color: var(--primary);
}
.add-plus {
  font-size: 15px;
}
.panel-foot {
  border-top: 1px solid var(--border);
  display: flex;
  gap: 8px;
  padding: 10px 12px;
}
.foot-btn {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  flex: 1;
  font-size: 11.5px;
  padding: 6px 0;
  transition: border-color 120ms ease, color 120ms ease;
}
.foot-btn:hover:not(:disabled) {
  border-color: var(--primary);
  color: var(--primary);
}
.foot-btn.danger:hover:not(:disabled) {
  border-color: var(--danger);
  color: var(--danger);
}
.foot-btn:disabled {
  cursor: default;
  opacity: 0.45;
}
.tabnum {
  font-variant-numeric: tabular-nums;
}
</style>
