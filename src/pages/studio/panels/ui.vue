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
import MenuFormBasics from "../components/menu-form/MenuFormBasics.vue";
import MenuFormTip from "../components/menu-form/MenuFormTip.vue";
import {
  Stepper,
  StepperIndicator,
  StepperItem,
  StepperSeparator,
  StepperTitle,
  StepperTrigger,
} from "@/components/ui/stepper";
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

/* 编辑流程：① 菜单条目（基础信息 + 槽位图标）→ ② 菜单提示（hover 弹窗内容） */
const step = ref<1 | 2>(1);
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
        <Stepper v-model="step" :linear="false" class="items-center gap-0">
          <StepperItem :step="1" class="flex-1 gap-0 pr-1">
            <StepperTrigger class="shrink-0 flex-row gap-2">
              <StepperIndicator />
              <StepperTitle class="whitespace-nowrap">菜单条目</StepperTitle>
            </StepperTrigger>
            <StepperSeparator class="min-w-4 flex-1" />
          </StepperItem>
          <StepperItem :step="2" class="flex-1 gap-0 pl-1">
            <StepperTrigger class="shrink-0 flex-row gap-2">
              <StepperIndicator />
              <StepperTitle class="whitespace-nowrap">菜单提示</StepperTitle>
            </StepperTrigger>
          </StepperItem>
        </Stepper>

        <div class="sheet-scroll">
          <MenuFormBasics
            v-show="step === 1"
            v-model="draft"
            @crop="cropperTarget = 'preview'"
          />
          <MenuFormTip v-show="step === 2" v-model="draft" @crop="cropperTarget = 'marquee'" />

          <div class="step-flow">
            <button v-if="step === 2" type="button" class="act" @click="step = 1">上一步</button>
            <button v-if="step === 1" type="button" class="act primary" @click="step = 2">
              下一步：菜单提示
            </button>
          </div>
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
.step-flow {
  display: flex;
  justify-content: flex-end;
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
