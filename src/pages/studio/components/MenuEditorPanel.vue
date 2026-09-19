<script setup lang="ts">
/**
 * 菜单编辑面板：显示当前选中菜单级的条目预览（图标 + 名称 + 排序）。
 * 点击条目 → 打开单条编辑 Sheet（由宿主页控制）；
 * 末位虚线加号块 → 追加新条目。底部为落库/还原动作。
 *
 * 条目缩略图优先用真实槽位图（kPropToolIconKey 提取物，toolPreview），
 * 无图时回退 FIcon 占位；hover 弹出游戏 BuildingRollover 样式的提示框
 * （标题 + rollover 大图 + 描述 + 锁定项的红色解锁提示）。
 */
import { computed, ref } from "vue";
import { storeToRefs } from "pinia";
import FIcon from "@/components/extensions/FIcon.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import { toolIconName, toolPreview, type WorkbenchTool } from "@/lib/game-ui/workbench";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const store = useUiWorkbenchStore();
const { activeMenu, edits } = storeToRefs(store);

const editedCount = computed(() => Object.keys(edits.value).length);

/** hover 提示框状态：锚定卡片中心 x + 卡片顶 y（fixed 定位，视口坐标）；
 * 上方放不下（提示框高约 320px）时翻到卡片下方。 */
const hovered = ref<{ tool: WorkbenchTool; x: number; y: number; below: boolean } | null>(null);
let hoverTimer = 0;

function onHoverStart(tool: WorkbenchTool, event: MouseEvent): void {
  const card = event.currentTarget as HTMLElement;
  window.clearTimeout(hoverTimer);
  hoverTimer = window.setTimeout(() => {
    const rect = card.getBoundingClientRect();
    const x = Math.max(160, Math.min(rect.left + rect.width / 2, window.innerWidth - 160));
    const below = rect.top < 340;
    hovered.value = { tool, x, y: below ? rect.bottom + 6 : rect.top - 6, below };
  }, 120);
}

function onHoverEnd(): void {
  window.clearTimeout(hoverTimer);
  hovered.value = null;
}

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

    <div v-else class="item-list" @mouseleave="onHoverEnd">
      <button
        v-for="entry in activeMenu.entries"
        :key="entry.tool.id"
        type="button"
        class="item-card"
        @click="emitEdit(entry.tool.id)"
        @mouseenter="onHoverStart(entry.tool, $event)"
        @mouseleave="onHoverEnd"
      >
        <span class="item-thumb">
          <img
            v-if="toolPreview(entry.tool)"
            class="item-img"
            :class="{ locked: entry.tool.locked }"
            :src="toolPreview(entry.tool) ?? undefined"
            alt=""
          />
          <FIcon v-else :name="toolIconName(entry.tool)" :size="22" aria-label="" />
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

    <!-- hover 提示框：按游戏 BuildingRollover 复刻（面板在 shadow DOM 外，
     * 游戏类不可用，样式本地复刻：白→浅灰渐变窗 + 钢蓝标题 + 大图 +
     * 图底半透明黑条描述 + 红色解锁提示）。 -->
    <Teleport to="body">
      <div
        v-if="hovered"
        class="game-rollover"
        :class="{ below: hovered.below }"
        :style="{ left: `${hovered.x}px`, top: `${hovered.y}px` }"
      >
        <div class="game-rollover-title">{{ hovered.tool.label }}</div>
        <div class="game-rollover-media">
          <img
            v-if="hovered.tool.marquee || hovered.tool.preview"
            :src="hovered.tool.marquee ?? hovered.tool.preview ?? undefined"
            alt=""
          />
        </div>
        <p v-if="hovered.tool.desc" class="game-rollover-desc">{{ hovered.tool.desc }}</p>
        <p
          v-if="hovered.tool.locked && hovered.tool.unlock"
          class="game-rollover-unlock"
        >
          {{ hovered.tool.unlock }}
        </p>
      </div>
    </Teleport>

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
.item-img {
  height: 100%;
  object-fit: contain;
  width: 100%;
}
.item-img.locked {
  filter: grayscale(0.4) brightness(0.95);
}

/* ── hover 提示框：按游戏 BuildingRollover 复刻（Teleport 到 body，fixed 定位） ── */
.game-rollover {
  background: linear-gradient(180deg, #fff 0%, #f5f5f5 100%);
  border-radius: 12px;
  box-shadow: 0 1px 7px rgb(0 0 0 / 50%);
  box-sizing: border-box;
  left: 0;
  overflow: hidden;
  padding: 0;
  position: fixed;
  top: 0;
  transform: translate(-50%, -100%);
  width: 300px;
  z-index: 60;
}
.game-rollover.below {
  transform: translate(-50%, 0);
}
.game-rollover-title {
  color: #33607d;
  font-size: 16px;
  font-weight: 700;
  padding: 10px 14px 8px;
  text-align: left;
}
.game-rollover-media {
  background: #dfe6ea;
  border-bottom: 1px solid #c6d2da;
  border-top: 1px solid #c6d2da;
  height: 158px;
  position: relative;
}
.game-rollover-media img {
  height: 100%;
  object-fit: cover;
  width: 100%;
}
.game-rollover-desc {
  background: rgb(0 0 0 / 78%);
  bottom: 0;
  color: #fff;
  font-size: 12px;
  left: 0;
  line-height: 1.35;
  margin: 0;
  padding: 6px 10px;
  position: absolute;
  right: 0;
  text-align: left;
}
.game-rollover-unlock {
  color: #d3242a;
  font-size: 12px;
  margin: 0;
  padding: 7px 10px;
  text-align: center;
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
