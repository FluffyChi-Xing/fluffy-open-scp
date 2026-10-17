<script setup lang="ts">
/**
 * UI 工作台：一站式完成资产在菜单中的落库与实时预览。
 *
 * 版面 = 左主区（复刻站渲染的游戏 HUD，1600×900 等比缩放）+ 右面板（当前菜单的
 * 条目预览 → 点击进 Sheet 编辑 → 实时反映回左区）。顶部工具条提供机制说明。
 * 落库当前导出 overlay JSON，后端写回走下一轮的
 * patch_property_overlay + 版本记录通道。
 */
import { computed, onMounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FImageCropper from "@/components/extensions/FImageCropper.vue";
import MenuSheetBody from "../components/menu-form/MenuSheetBody.vue";
import type { ReplicaSubmenuItem } from "@/lib/game-ui/replica/types";
import FTypography from "@/components/extensions/FTypography.vue";
import FCode from "@/components/ui/FCode.vue";
import FSheet from "@/components/ui/FSheet.vue";
import UiWorkbenchStage from "../components/UiWorkbenchStage.vue";
import MenuEditorPanel from "../components/MenuEditorPanel.vue";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import { tauriApi } from "@/api";
import { downloadBytes } from "@/lib/game-ui/menu-export";
import type { ToolEdit } from "@/lib/game-ui/workbench";

const { t } = useI18n();
const store = useUiWorkbenchStore();
const { loading, editingEntry } = storeToRefs(store);

onMounted(() => {
  void store.load();
});

const showMechanism = ref(false);
/** 当前覆盖数量（工具条按钮可用性）。 */
const editedCount = computed(
  () =>
    Object.keys(store.edits).length +
    Object.values(store.added).reduce((count, list) => count + list.length, 0) +
    store.customCategories.length,
);
/** schema diff 预览 sheet */
const showDiff = ref(false);
const diffText = computed(() => store.schemaDiffText());

/** bytes → base64（Tauri 命令传输用）。 */
function toBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

/** 编译 .package 并注入游戏 Packages 目录；非 Tauri 环境回退为下载。 */
async function onBuildInject(): Promise<void> {
  const result = store.buildGameExport();
  if (result.entryCount === 0) {
    window.alert(t("studio.workbench.buildEmpty"));
    return;
  }
  try {
    const deployed = await tauriApi.workspace.deployUiOverlay(toBase64(result.bytes));
    let message = t("studio.workbench.injectSuccess", { dest: deployed.destination });
    if (deployed.gameRunning) message += "\n" + t("studio.workbench.injectGameRunning");
    window.alert(message);
  } catch {
    downloadBytes(result.bytes, "OpenSCP-ui-overlay.package", "application/octet-stream");
    window.alert(t("studio.workbench.injectFallback"));
  }
}

/** Sheet 的临时编辑态（打开时从条目拷贝，确认才写回 store）。 */
const draft = ref<{
  label: string;
  icon: string;
  pos: number;
  desc: string;
  cost: string;
  upkeep: string;
  unlock: string;
  locked: boolean;
  preview: string | null;
  marquee: string | null;
  stats: { label: string; value: string }[];
}>({
  label: "",
  icon: "",
  pos: 0,
  desc: "",
  cost: "",
  upkeep: "",
  unlock: "",
  locked: false,
  preview: null,
  marquee: null,
  stats: [],
});

/** Sheet 打开时从条目**拷贝**到临时编辑态（编辑副本；应用才写回覆盖）。
 * preview/marquee 带完整值（原资产路径或已裁切 dataURL），移除 = 置 null。 */
watch(editingEntry, (entry) => {
  if (entry) {
    draft.value = {
      label: entry.tool.label,
      icon: entry.tool.icon ?? "",
      pos: entry.tool.pos,
      desc: entry.tool.desc ?? "",
      cost: entry.tool.cost ?? "",
      upkeep: entry.tool.upkeep ?? "",
      unlock: entry.tool.unlock ?? "",
      locked: entry.tool.locked ?? false,
      preview: entry.tool.preview ?? null,
      marquee: entry.tool.marquee ?? null,
      stats: (entry.tool.stats ?? []).map((stat) => ({
        label: stat.label,
        value: stat.value ?? "",
      })),
    };
  }
});

/** 条目当前归属的菜单（「指哪打哪」：编辑前先看清它挂在哪）。 */
const memberships = computed(() =>
  editingEntry.value ? store.membershipsOf(editingEntry.value.tool.id) : [],
);
/** 复制目标候选：真实容器（当前视图 + 跨视图的大学子菜单）与自定义菜单。 */
const copyTargets = computed(() =>
  editingEntry.value ? store.copyTargetMenus(editingEntry.value.menuId) : [],
);

/** 当前条目的建筑子菜单（costs.json 反查产物；无则不显示）。 */
const submenus = computed(() =>
  editingEntry.value ? store.replica?.submenus?.[editingEntry.value.tool.id] : undefined,
);

/** 打开子菜单条目（压栈，形成嵌套 Sheet）。 */
function onOpenSubmenu(item: ReplicaSubmenuItem): void {
  store.openSubmenuItem(item);
}

/** 复制到目标菜单并立即编辑副本（SCP Copy Entry 语义）。 */
function onCopyTo(targetId: string): void {
  const entry = editingEntry.value;
  if (!entry) return;
  const created = store.duplicateToMenu(entry.menuId, entry.tool.id, targetId);
  if (!created) return;
  store.enterMenu(targetId);
  store.editingEntry = created;
}

/** 应用 = 计算草稿相对条目有效视图的 diff，只把变化的字段写进覆盖。
 * 图片三态：未动 → 不写键；null → 明确移除；字符串 → 替换。 */
function applyDraft(): void {
  const entry = editingEntry.value;
  if (!entry) return;
  const tool = entry.tool;
  const patch: ToolEdit = {};
  if (draft.value.label !== tool.label) patch.label = draft.value.label;
  const draftIcon = draft.value.icon === "" ? null : draft.value.icon;
  if (draftIcon !== (tool.icon ?? null)) patch.icon = draftIcon;
  const draftPos = Number(draft.value.pos) || 0;
  if (draftPos !== tool.pos) patch.pos = draftPos;
  if (draft.value.desc !== (tool.desc ?? "")) patch.desc = draft.value.desc;
  const draftCost = draft.value.cost === "" ? null : draft.value.cost;
  if (draftCost !== (tool.cost ?? null)) patch.cost = draftCost;
  const draftUpkeep = draft.value.upkeep === "" ? null : draft.value.upkeep;
  if (draftUpkeep !== (tool.upkeep ?? null)) patch.upkeep = draftUpkeep;
  if (draft.value.unlock !== (tool.unlock ?? "")) patch.unlock = draft.value.unlock;
  if (draft.value.locked !== (tool.locked ?? false)) patch.locked = draft.value.locked;
  if (draft.value.preview !== (tool.preview ?? null)) patch.preview = draft.value.preview;
  if (draft.value.marquee !== (tool.marquee ?? null)) patch.marquee = draft.value.marquee;
  const draftStats = draft.value.stats.map((stat) => ({
    label: stat.label,
    value: stat.value === "" ? null : stat.value,
  }));
  if (JSON.stringify(draftStats) !== JSON.stringify(tool.stats ?? [])) patch.stats = draftStats;
  if (Object.keys(patch).length > 0) store.updateItem(entry.menuId, entry.tool.id, patch);
  store.closeEditor();
}

/* 裁切器：icon 128×128 PNG、hover 大图 454×263 JPEG（游戏资源实测尺寸） */
const cropperTarget = ref<"preview" | "marquee" | null>(null);
const cropperPresets = computed(() => ({
  preview: { width: 128, height: 128, format: "image/png" as const, title: t("studio.workbench.cropperPreviewTitle") },
  marquee: { width: 454, height: 263, format: "image/jpeg" as const, title: t("studio.workbench.cropperMarqueeTitle") },
}));
const cropperActive = computed(() =>
  cropperTarget.value === "marquee" ? cropperPresets.value.marquee : cropperPresets.value.preview,
);
function onCropped(dataUrl: string): void {
  if (cropperTarget.value === "preview") draft.value.preview = dataUrl;
  if (cropperTarget.value === "marquee") draft.value.marquee = dataUrl;
  cropperTarget.value = null;
}

function removeDraft(): void {
  const entry = editingEntry.value;
  if (!entry || entry.isSubmenu) return;
  store.removeItem(entry.menuId, entry.tool.id);
  store.resetEditor();
}

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
      <div class="view-switch" role="group" :aria-label="t('studio.workbench.viewMode')">
        <button
          v-for="mode in (['city', 'region', 'development'] as const)"
          :key="mode"
          type="button"
          :aria-pressed="store.viewMode === mode"
          :class="{ selected: store.viewMode === mode }"
          @click="store.setViewMode(mode)"
        >{{ t(`studio.workbench.dim${mode === 'city' ? 'City' : mode === 'region' ? 'Region' : 'Development'}`) }}</button>
      </div>
      <button type="button" class="ghost-btn" @click="showMechanism = !showMechanism">
        <FIcon :name="showMechanism ? 'ChevronUp' : 'ChevronDown'" :size="13" aria-label="" />
        {{ t("studio.workbench.mechanism") }}
      </button>
      <div class="toolbar-spacer">
        <button type="button" class="ghost-btn" :disabled="!editedCount" @click="showDiff = true">
          <FIcon name="GitCompare" :size="13" aria-label="" />
          {{ t("studio.workbench.schemaDiff") }}
        </button>
        <button type="button" class="ghost-btn primary" :disabled="!editedCount" @click="onBuildInject">
          <FIcon name="Package" :size="13" aria-label="" />
          {{ t("studio.workbench.buildInject") }}
        </button>
      </div>
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
      <UiWorkbenchStage />
      <MenuEditorPanel />
    </div>

    <!-- 单条菜单项编辑（子菜单条目嵌套：栈式返回上级） -->
    <FSheet
      :open="!!editingEntry"
      width="420px"
      :label="t('studio.workbench.sheetTitle')"
      @update:open="!$event && store.closeEditor()"
    >
      <div v-if="editingEntry" class="sheet-body">
        <button
          v-if="store.editorStack.length > 0"
          type="button"
          class="back-parent"
          @click="store.closeEditor()"
        >
          <FIcon name="ArrowLeft" :size="13" aria-label="" />
          {{ t('studio.workbench.backToParent') }}
        </button>
        <p class="sheet-id">
          {{ t("studio.workbench.sheetId") }}:
          <code>{{ editingEntry.tool.id }}</code>
        </p>

        <div class="sheet-scroll">
          <MenuSheetBody
            v-model="draft"
            :memberships="memberships"
            :copy-targets="copyTargets"
            :submenus="submenus"
            :nested="!!editingEntry.isSubmenu"
            @crop="cropperTarget = $event"
            @copy="onCopyTo"
            @open-submenu="onOpenSubmenu"
          />
        </div>

        <div class="sheet-actions">
          <button
            v-if="!editingEntry.isSubmenu"
            type="button"
            class="act danger"
            @click="removeDraft"
          >
            {{ editingEntry.isNew ? t("studio.workbench.discard") : t("studio.workbench.removeEntry") }}
          </button>
          <button v-else type="button" class="act danger" @click="store.closeEditor()">
            {{ t('studio.workbench.backToParent') }}
          </button>
          <button type="button" class="act primary" @click="applyDraft">{{ t('studio.workbench.apply') }}</button>
        </div>
      </div>
    </FSheet>

    <!-- schema diff 预览 -->
    <FSheet
      :open="showDiff"
      width="640px"
      :label="t('studio.workbench.diffTitle')"
      @update:open="!$event && (showDiff = false)"
    >
      <div class="diff-body">
        <FCode
          :code="diffText || t('studio.workbench.diffEmpty')"
          :copy-label="t('code.copy')"
          :copied-label="t('code.copied')"
          :collapse-label="t('code.collapse')"
          :expand-label="t('code.expand')"
        />
      </div>
    </FSheet>

    <!-- 图片裁切器：槽位图标 128×128 PNG / hover 大图 454×263 JPEG -->
    <FImageCropper
      :open="cropperTarget !== null"
      :width="cropperActive.width"
      :height="cropperActive.height"
      :format="cropperActive.format"
      :title="cropperActive.title"
      @update:open="!$event && (cropperTarget = null)"
      @cropped="onCropped"
    />
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
.toolbar-spacer {
  display: inline-flex;
  gap: 8px;
  margin-inline-start: auto;
}
.ghost-btn.primary {
  background: var(--primary);
  border-color: var(--primary);
  color: var(--primary-foreground);
  font-weight: 600;
}
.ghost-btn.primary:hover {
  filter: brightness(1.08);
}
.ghost-btn:disabled {
  cursor: default;
  opacity: 0.45;
}
.diff-body {
  padding: 16px 20px 20px;
}
.diff-body :deep(.f-code) {
  max-height: 60vh;
  overflow: auto;
}
.view-switch {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: inline-flex;
  overflow: hidden;
}
.view-switch button {
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  font-size: 12px;
  padding: 7px 12px;
}
.view-switch button.selected {
  background: var(--primary);
  color: var(--primary-foreground);
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

/* Sheet 编辑表单 */
.back-parent {
  align-items: center;
  align-self: flex-start;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font-size: 11.5px;
  gap: 5px;
  padding: 4px 10px;
}
.back-parent:hover {
  border-color: var(--primary);
  color: var(--primary);
}
.sheet-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  overflow: hidden;
  padding: 20px 20px 16px;
}
.sheet-scroll {
  display: flex;
  flex-direction: column;
  flex: 1;
  gap: 12px;
  margin: 0 -20px;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 20px 12px;
}
.sheet-actions {
  border-top: 1px solid var(--border);
  display: grid;
  gap: 8px;
  grid-template-columns: 1fr 1fr 1fr;
  padding-top: 12px;
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
  gap: 4px;
  min-width: 0;
}
.field > span {
  color: var(--muted-foreground);
}
.field input:not([type="checkbox"]),
.field textarea {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 13px;
  height: 32px;
  min-width: 0;
  padding: 5px 9px;
  width: 100%;
}
.field textarea {
  height: auto;
  resize: vertical;
}
.field-check {
  align-items: center;
  display: flex;
  flex-direction: row;
  gap: 6px;
}
.field-check span {
  color: var(--foreground);
  font-size: 12px;
}
.field-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 4px 0 0;
}
.img-btn {
  align-items: center;
  background: var(--surface);
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  height: 74px;
  justify-content: center;
  overflow: hidden;
  width: 100%;
}
.img-btn.square {
  width: 74px;
}
.img-btn:hover {
  border-color: var(--primary, #0b78fe);
}
.img-btn img {
  height: 100%;
  object-fit: contain;
  width: 100%;
}
.img-btn-hint {
  font-size: 11px;
  padding: 0 4px;
  text-align: center;
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
.preview-img.locked {
  filter: grayscale(0.4) brightness(0.95);
}
.preview-label {
  font-size: 13px;
  font-weight: 600;
}
.act {
  align-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: inline-flex;
  font-size: 12px;
  height: 34px;
  justify-content: center;
  padding: 0 6px;
  white-space: nowrap;
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
.field-check {
  align-items: center;
  flex-direction: row;
  gap: 6px;
}
.field textarea {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 13px;
  padding: 7px 9px;
  resize: vertical;
}
.img-btn {
  align-items: center;
  background: var(--surface);
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  height: 64px;
  justify-content: center;
  overflow: hidden;
  width: 64px;
}
.img-btn.wide {
  width: 110px;
}
.img-btn img {
  height: 100%;
  object-fit: contain;
  width: 100%;
}
.preview-label {
  font-size: 13px;
  font-weight: 600;
}
</style>
