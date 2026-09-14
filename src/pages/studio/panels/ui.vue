<script setup lang="ts">
/**
 * UI 工作台：一站式完成资产在菜单中的落库与实时预览。
 *
 * 版面 = 左主区（重建的游戏 HUD，1600×900 等比缩放）+ 右面板（当前菜单的
 * 条目预览 → 点击进 Sheet 编辑 → 实时反映回左区）。顶部工具条提供
 * 屏幕切换（城市主菜单 / 大学建筑菜单）、参考截图叠加（校准）与
 * 机制说明。落库当前导出 overlay JSON，后端写回走下一轮的
 * patch_property_overlay + 版本记录通道。
 */
import { computed, onMounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FCode from "@/components/ui/FCode.vue";
import FSheet from "@/components/ui/FSheet.vue";
import UiWorkbenchStage from "../components/UiWorkbenchStage.vue";
import MenuEditorPanel from "../components/MenuEditorPanel.vue";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import type { ToolEdit } from "@/lib/game-ui/workbench";

const { t } = useI18n();
const store = useUiWorkbenchStore();
const { screen, loading, editingEntry, data } = storeToRefs(store);

onMounted(() => {
  void store.load();
});

/** 参考截图叠加透明度（0 = 关闭）。 */
const overlayOpacity = ref(0);
const showMechanism = ref(false);

/** Sheet 的临时编辑态（打开时从条目拷贝，确认才写回 store）。 */
const draft = ref<{ label: string; icon: string; pos: number }>({
  label: "",
  icon: "",
  pos: 0,
});

/** Sheet 打开时从条目拷贝到临时编辑态。 */
watch(editingEntry, (entry) => {
  if (entry) {
    draft.value = {
      label: entry.tool.label,
      icon: entry.tool.icon ?? "",
      pos: entry.tool.pos,
    };
  }
});

function applyDraft(): void {
  const entry = editingEntry.value;
  if (!entry) return;
  const patch: ToolEdit = {
    label: draft.value.label,
    icon: draft.value.icon === "" ? null : draft.value.icon,
    pos: Number(draft.value.pos) || 0,
  };
  store.updateItem(entry.menuId, entry.tool.id, patch);
  editingEntry.value = null;
}

function removeDraft(): void {
  const entry = editingEntry.value;
  if (!entry) return;
  store.removeItem(entry.menuId, entry.tool.id);
  editingEntry.value = null;
}

const screens = computed(() => [
  { id: "city" as const, label: t("studio.workbench.screenCity") },
  { id: "university" as const, label: t("studio.workbench.screenUniversity") },
]);
</script>

<template>
  <section class="workbench-page">
    <div class="page-head-row">
      <RouterLink class="back-link" to="/studio">
        <FIcon name="ArrowLeft" :size="14" aria-label="" />
        {{ t("version.backToStudio") }}
      </RouterLink>

      <header class="page-header">
        <span class="page-icon"><FIcon name="PanelTop" :size="20" aria-label="" /></span>
        <div>
          <FTypography :header="2" spacing="none">{{ t("studio.workbench.title") }}</FTypography>
          <p class="page-meta">{{ t("studio.workbench.description") }}</p>
        </div>
      </header>
    </div>

    <p v-if="loading" class="notice" role="status">{{ t("studio.workbench.loading") }}</p>

    <div class="toolbar">
      <div class="screen-switch" role="tablist">
        <button
          v-for="item in screens"
          :key="item.id"
          type="button"
          class="screen-tab"
          :class="{ active: screen === item.id }"
          role="tab"
          :aria-selected="screen === item.id"
          @click="store.selectScreen(item.id)"
        >
          {{ item.label }}
        </button>
      </div>

      <label class="overlay-row">
        <span>{{ t("studio.workbench.overlay") }}</span>
        <input
          v-model.number="overlayOpacity"
          type="range"
          min="0"
          max="100"
          :disabled="!data"
        />
        <span class="tabnum">{{ overlayOpacity }}%</span>
      </label>

      <button type="button" class="ghost-btn" @click="showMechanism = !showMechanism">
        <FIcon :name="showMechanism ? 'ChevronUp' : 'ChevronDown'" :size="13" aria-label="" />
        {{ t("studio.workbench.mechanism") }}
      </button>
    </div>

    <div v-if="showMechanism" class="mechanism">
      <FCode
        :code="t('studio.workbench.mechanismBody')"
        :copy-label="t('code.copy')"
        :copied-label="t('code.copied')"
        :collapse-label="t('code.collapse')"
        :expand-label="t('code.expand')"
      />
    </div>

    <div class="workspace">
      <UiWorkbenchStage :overlay-opacity="overlayOpacity / 100" />
      <MenuEditorPanel />
    </div>

    <!-- 单条菜单项编辑 -->
    <FSheet
      :open="!!editingEntry"
      :label="t('studio.workbench.sheetTitle')"
      @update:open="!$event && (editingEntry = null)"
    >
      <div v-if="editingEntry" class="sheet-body">
        <p class="sheet-id">
          {{ t("studio.workbench.sheetId") }}:
          <code>{{ editingEntry.tool.id }}</code>
        </p>
        <label class="field">
          <span>{{ t("studio.workbench.fieldLabel") }}</span>
          <input v-model="draft.label" type="text" />
        </label>
        <label class="field">
          <span>{{ t("studio.workbench.fieldIcon") }}</span>
          <input v-model="draft.icon" type="text" placeholder="Box" />
        </label>
        <p class="field-hint">{{ t("studio.workbench.iconHint") }}</p>
        <label class="field">
          <span>{{ t("studio.workbench.fieldPos") }}</span>
          <input v-model.number="draft.pos" type="number" />
        </label>
        <div class="sheet-preview">
          <span class="preview-thumb">
            <FIcon :name="draft.icon.trim() === '' ? 'Box' : draft.icon" :size="22" aria-label="" />
          </span>
          <span class="preview-label">{{ draft.label }}</span>
        </div>
        <div class="sheet-actions">
          <button type="button" class="act danger" @click="removeDraft">
            {{ editingEntry.isNew ? t("studio.workbench.discard") : t("studio.workbench.removeEdit") }}
          </button>
          <button type="button" class="act primary" @click="applyDraft">
            {{ t("studio.workbench.apply") }}
          </button>
        </div>
      </div>
    </FSheet>
  </section>
</template>

<style scoped>
.workbench-page {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding-bottom: 2rem;
}
.page-head-row {
  align-items: center;
  display: flex;
  gap: 18px;
}
.back-link {
  align-items: center;
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 12px;
  gap: 6px;
  text-decoration: none;
  width: fit-content;
}
.back-link:hover {
  color: var(--foreground);
}
.page-header {
  align-items: center;
  display: flex;
  gap: 1rem;
}
.page-header :deep(h2) {
  margin: 0;
}
.page-icon {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: var(--brand);
  display: inline-flex;
  flex-shrink: 0;
  height: 44px;
  justify-content: center;
  width: 44px;
}
.page-meta {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 3px 0 0;
}
.notice {
  background: var(--surface-hover);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
  padding: 8px 10px;
}
.toolbar {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
}
.screen-switch {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: inline-flex;
  overflow: hidden;
}
.screen-tab {
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  font-size: 12px;
  padding: 7px 16px;
  transition: background-color 120ms ease, color 120ms ease;
}
.screen-tab.active {
  background: var(--primary);
  color: var(--primary-foreground);
  font-weight: 700;
}
.overlay-row {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 12px;
  gap: 8px;
}
.overlay-row input[type="range"] {
  accent-color: var(--primary);
  width: 140px;
}
.ghost-btn {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font-size: 11.5px;
  gap: 5px;
  padding: 6px 10px;
}
.ghost-btn:hover {
  border-color: var(--border-strong);
  color: var(--foreground);
}
.mechanism {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 10px 12px;
}
.workspace {
  /* 固定高度：重建舞台与面板互不影响（面板内自行滚动） */
  display: grid;
  gap: 14px;
  grid-template-columns: minmax(0, 1fr) 320px;
  height: 660px;
}
.tabnum {
  font-variant-numeric: tabular-nums;
}

/* Sheet 编辑表单 */
.sheet-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 24px;
}
.sheet-id {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
}
.sheet-id code {
  background: var(--surface-hover);
  border-radius: var(--radius-sm);
  padding: 1px 6px;
}
.field {
  display: flex;
  flex-direction: column;
  font-size: 12px;
  gap: 5px;
}
.field span {
  color: var(--muted-foreground);
}
.field input {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font-size: 13px;
  padding: 7px 9px;
}
.field-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: -6px 0 0;
}
.sheet-preview {
  align-items: center;
  background: var(--surface-hover);
  border-radius: var(--radius-md);
  display: flex;
  gap: 10px;
  padding: 10px;
}
.preview-thumb {
  align-items: center;
  background: var(--surface);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: inline-flex;
  height: 48px;
  justify-content: center;
  overflow: hidden;
  width: 48px;
}
.preview-thumb img {
  height: 100%;
  object-fit: contain;
  width: 100%;
}
.preview-label {
  font-size: 13px;
  font-weight: 600;
}
.sheet-actions {
  display: flex;
  gap: 10px;
  justify-content: flex-end;
}
.act {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: 12px;
  padding: 7px 14px;
}
.act.primary {
  background: var(--primary);
  border-color: var(--primary);
  color: var(--primary-foreground);
  font-weight: 700;
}
.act.primary:hover {
  background: var(--primary-hover);
}
.act.danger {
  background: var(--surface);
  color: var(--danger);
}
.act.danger:hover {
  border-color: var(--danger);
}
</style>
