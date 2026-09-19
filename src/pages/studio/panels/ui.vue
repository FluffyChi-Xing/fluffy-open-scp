<script setup lang="ts">
/**
 * UI 工作台：一站式完成资产在菜单中的落库与实时预览。
 *
 * 版面 = 左主区（复刻站渲染的游戏 HUD，1600×900 等比缩放）+ 右面板（当前菜单的
 * 条目预览 → 点击进 Sheet 编辑 → 实时反映回左区）。顶部工具条提供机制说明。
 * 落库当前导出 overlay JSON，后端写回走下一轮的
 * patch_property_overlay + 版本记录通道。
 */
import { onMounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FImageCropper from "@/components/extensions/FImageCropper.vue";
import { downloadBytes } from "@/lib/game-ui/menu-export";
import FTypography from "@/components/extensions/FTypography.vue";
import FCode from "@/components/ui/FCode.vue";
import FSheet from "@/components/ui/FSheet.vue";
import UiWorkbenchStage from "../components/UiWorkbenchStage.vue";
import MenuEditorPanel from "../components/MenuEditorPanel.vue";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import type { ToolEdit } from "@/lib/game-ui/workbench";

const { t } = useI18n();
const store = useUiWorkbenchStore();
const { loading, editingEntry } = storeToRefs(store);

onMounted(() => {
  void store.load();
});

const showMechanism = ref(false);

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
});

/** Sheet 打开时从条目拷贝到临时编辑态。 */
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
      preview: typeof entry.tool.preview === "string" && entry.tool.preview.startsWith("data:") ? entry.tool.preview : null,
      marquee: typeof entry.tool.marquee === "string" && entry.tool.marquee.startsWith("data:") ? entry.tool.marquee : null,
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
    desc: draft.value.desc,
    cost: draft.value.cost === "" ? null : draft.value.cost,
    upkeep: draft.value.upkeep === "" ? null : draft.value.upkeep,
    unlock: draft.value.unlock,
    locked: draft.value.locked,
    // 未裁切时传 undefined（不产生覆盖），避免把基础资产路径抹掉
    preview: draft.value.preview ?? undefined,
    marquee: draft.value.marquee ?? undefined,
  };
  store.updateItem(entry.menuId, entry.tool.id, patch);
  editingEntry.value = null;
}

/* 裁切器：icon 128×128 PNG、hover 大图 454×263 JPEG（游戏资源实测尺寸） */
const cropperTarget = ref<"preview" | "marquee" | null>(null);
const cropperPresets = {
  preview: { width: 128, height: 128, format: "image/png" as const, title: "槽位图标（128×128 PNG）" },
  marquee: { width: 454, height: 263, format: "image/jpeg" as const, title: "Hover 大图（454×263 JPEG）" },
};
function onCropped(dataUrl: string): void {
  if (cropperTarget.value === "preview") draft.value.preview = dataUrl;
  if (cropperTarget.value === "marquee") draft.value.marquee = dataUrl;
}

/** 单条菜单条目 → .property 资源下载（仅既有游戏条目可导）。 */
function exportProperty(): void {
  const entry = editingEntry.value;
  if (!entry) return;
  const bytes = store.buildEntryProperty(entry.tool.id);
  if (!bytes) {
    window.alert("新增条目还没有游戏 instance，请使用「导出 .package」");
    return;
  }
  downloadBytes(bytes, `${entry.tool.id}.property`);
}

function removeDraft(): void {
  const entry = editingEntry.value;
  if (!entry) return;
  store.removeItem(entry.menuId, entry.tool.id);
  editingEntry.value = null;
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
      <UiWorkbenchStage />
      <MenuEditorPanel />
    </div>

    <!-- 单条菜单项编辑 -->
    <FSheet
      :open="!!editingEntry"
      width="420px"
      :label="t('studio.workbench.sheetTitle')"
      @update:open="!$event && (editingEntry = null)"
    >
      <div v-if="editingEntry" class="sheet-body">
        <p class="sheet-id">
          {{ t("studio.workbench.sheetId") }}:
          <code>{{ editingEntry.tool.id }}</code>
        </p>
        <section class="sheet-section">
          <p class="section-title">基础</p>
          <div class="field-grid">
            <label class="field span-2">
              <span>显示名称</span>
              <input v-model="draft.label" type="text" />
            </label>
            <label class="field">
              <span>排序（uiPosition）</span>
              <input v-model.number="draft.pos" type="number" />
            </label>
            <label class="field">
              <span>图标（FIcon 名，可选）</span>
              <input v-model="draft.icon" type="text" placeholder="Box" />
            </label>
          </div>
          <p class="field-hint">{{ t("studio.workbench.iconHint") }}</p>
        </section>

        <section class="sheet-section">
          <p class="section-title">文案与经济</p>
          <div class="field-grid">
            <label class="field span-2">
              <span>Hover 文案（描述）</span>
              <textarea v-model="draft.desc" rows="3" />
            </label>
            <label class="field">
              <span>造价 §</span>
              <input v-model="draft.cost" type="text" placeholder="27,500" />
            </label>
            <label class="field">
              <span>预算/小时 §</span>
              <input v-model="draft.upkeep" type="text" placeholder="-856" />
            </label>
            <label class="field span-2">
              <span>解锁提示</span>
              <input v-model="draft.unlock" type="text" />
            </label>
            <label class="field-check span-2">
              <input v-model="draft.locked" type="checkbox" />
              <span>锁定（hardGate，未批准/達到上限）</span>
            </label>
          </div>
        </section>

        <section class="sheet-section">
          <p class="section-title">图像（上传后裁切到游戏尺寸）</p>
          <div class="field-grid">
            <div class="field">
              <span>槽位图标 128×128</span>
              <button type="button" class="img-btn square" @click="cropperTarget = 'preview'">
                <img v-if="draft.preview" :src="draft.preview" alt="" />
                <span v-else class="img-btn-hint">上传/裁切</span>
              </button>
            </div>
            <div class="field">
              <span>Hover 大图 454×263</span>
              <button type="button" class="img-btn wide" @click="cropperTarget = 'marquee'">
                <img v-if="draft.marquee" :src="draft.marquee" alt="" />
                <span v-else class="img-btn-hint">上传/裁切</span>
              </button>
            </div>
          </div>
        </section>

        <div class="sheet-preview">
          <span class="preview-thumb">
            <img
              v-if="draft.icon.trim() === '' && editingEntry.tool.preview"
              class="preview-img"
              :class="{ locked: editingEntry.tool.locked }"
              :src="editingEntry.tool.preview"
              alt=""
            />
            <FIcon v-else :name="draft.icon.trim() === '' ? 'Box' : draft.icon" :size="22" aria-label="" />
          </span>
          <span class="preview-label">{{ draft.label }}</span>
        </div>
        <div class="sheet-actions">
          <button type="button" class="act" @click="exportProperty">导出 property</button>
          <button type="button" class="act danger" @click="removeDraft">
            {{ editingEntry.isNew ? t("studio.workbench.discard") : "还原" }}
          </button>
          <button type="button" class="act primary" @click="applyDraft">应用</button>
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
.sheet-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 20px 24px;
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
.sheet-section {
  border-top: 1px solid var(--border);
  padding-top: 12px;
}
.sheet-section:first-of-type {
  border-top: none;
  padding-top: 0;
}
.section-title {
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  margin: 0 0 8px;
}
.field-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 12px;
}
.field {
  display: flex;
  flex-direction: column;
  font-size: 12px;
  gap: 4px;
  min-width: 0;
}
.span-2 {
  grid-column: 1 / -1;
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
.sheet-actions {
  display: grid;
  gap: 8px;
  grid-template-columns: 1fr 1fr 1fr;
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
