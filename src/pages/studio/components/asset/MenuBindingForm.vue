<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { command } from "@/api/tauri";
import { tauriApi } from "@/api";
import { useToast } from "@/composables/useToast";
import FIcon from "@/components/extensions/FIcon.vue";
import FSelect from "@/components/ui/FSelect.vue";
import {
  menuBindingIssues,
  menuTargetsFor,
  newMenuEntry,
  type MenuDimension,
  type WorkbenchMenuBinding,
  type WorkbenchMenuEntry,
} from "./menuBinding";

/**
 * 菜单绑定表单（项目配置页 / 画布菜单节点共用语义）：
 * 维度 → 一级菜单容器 → 标题/介绍/图标/背景/造价。
 * 数据模型见 menuBinding.ts；构建时由 workbench_logic.rs 投影成 semantics.menu。
 */
const props = defineProps<{
  menu: WorkbenchMenuBinding;
  disabled?: boolean;
  /** 紧凑模式（画布节点内嵌）；默认表单模式。 */
  compact?: boolean;
  /** 项目相对根：提供后显示图标/背景图选择按钮（导入 assets/textures）。 */
  project?: string;
}>();
const emit = defineEmits<{
  "update:menu": [patch: Partial<WorkbenchMenuBinding>];
}>();

const { t } = useI18n();
const toast = useToast();
const importingImage = ref(false);

const dimensionOptions = computed(() => [
  { value: "city", label: t("studio.workbench.dimCity") },
  { value: "development", label: t("studio.workbench.dimDevelopment") },
]);

function entryTargetOptions(dimension: MenuDimension) {
  return [
    { value: "", label: t("studio.assetWorkbench.menuCategoryRequired") },
    ...menuTargetsFor(dimension).map((option) => ({
      value: option.id,
      label: t(option.labelKey),
    })),
  ];
}


function addEntry(): void {
  emit("update:menu", {
    entries: [...props.menu.entries, newMenuEntry(props.menu.entries)],
  });
}

function removeEntry(id: string): void {
  emit("update:menu", {
    entries: props.menu.entries.filter((entry) => entry.id !== id),
  });
}

function patchEntry(id: string, patch: Partial<WorkbenchMenuEntry>): void {
  emit("update:menu", {
    entries: props.menu.entries.map((entry) =>
      entry.id === id ? { ...entry, ...patch } : entry,
    ),
  });
}

function onDimension(entry: WorkbenchMenuEntry, dimension: string): void {
  if (dimension === entry.dimension) return;
  patchEntry(entry.id, {
    dimension: dimension as MenuDimension,
    categoryInstance: "",
  });
}

function onPosition(event: Event): void {
  const raw = (event.target as HTMLInputElement).value.trim();
  const value = raw === "" ? null : Number(raw);
  emit("update:menu", {
    uiPosition: value !== null && Number.isFinite(value) ? value : null,
  });
}

function onCost(field: "constructionCost" | "maintenanceCost", event: Event): void {
  const raw = (event.target as HTMLInputElement).value.trim();
  const value = raw === "" ? null : Number(raw);
  emit("update:menu", {
    [field]: value !== null && Number.isFinite(value) ? value : null,
  });
}

/** 上传框：选图 → 直接落入项目 assets/images 固定默认名（原图文件名不参与）。 */
async function uploadMenuImage(field: "icon" | "marquee"): Promise<void> {
  if (!props.project || importingImage.value) return;
  const path = await tauriApi.workspace.pickImageFile(
    t(
      field === "icon"
        ? "studio.assetWorkbench.menuIcon"
        : "studio.assetWorkbench.menuMarquee",
    ),
  );
  if (typeof path !== "string") return;
  importingImage.value = true;
  try {
    const imported = await command<{ asset: string }>("code_flow", {
      request: {
        project: props.project,
        action: "import-menu-image",
        payload: { path, kind: field },
      },
    });
    emit("update:menu", {
      [field === "icon" ? "iconPng" : "marqueePng"]: imported.asset,
    });
  } catch (cause) {
    toast.error(t("studio.assetWorkbench.importFailed"));
    console.warn("[menu] image upload failed:", cause);
  } finally {
    importingImage.value = false;
  }
}

/** 图片预览 = read-menu-image 返回的 base64 data URL。
 * 预览不走 asset 协议（scope 仅 $TEMP，项目目录图片必 403）。 */
const iconPreview = ref("");
const marqueePreview = ref("");
async function refreshImagePreviews(): Promise<void> {
  if (!props.project) return;
  try {
    for (const kind of ["icon", "marquee"] as const) {
      const result = await command<{ data: string | null }>("code_flow", {
        request: {
          project: props.project,
          action: "read-menu-image",
          payload: { kind },
        },
      });
      if (kind === "icon") iconPreview.value = result.data ?? "";
      else marqueePreview.value = result.data ?? "";
    }
  } catch (cause) {
    console.warn("[menu] image preview failed:", cause);
  }
}
watch(
  () => [props.menu.iconPng, props.menu.marqueePng] as const,
  () => {
    void refreshImagePreviews();
  },
  { immediate: true },
);

const issues = computed(() => menuBindingIssues(props.menu));
function issueLabel(code: string): string {
  const [base, id] = code.split(":");
  return t(`studio.assetWorkbench.menuIssue.${base}`, {
    id: id ? t("studio.assetWorkbench.menuEntryShort", { id }) : "",
  });
}
</script>

<template>
  <div class="menu-form" :class="{ compact }">

    <div class="mf-grid">
        <label class="mf-field">
          <span>{{ t("studio.assetWorkbench.menuLabel") }}</span>
          <input
            :value="menu.label"
            type="text"
            :disabled="disabled"
            :placeholder="t('studio.assetWorkbench.menuLabelPlaceholder')"
            @input="
              emit('update:menu', {
                label: ($event.target as HTMLInputElement).value,
              })
            "
          />
        </label>
        <label class="mf-field">
          <span>{{ t("studio.assetWorkbench.menuDescription") }}</span>
          <input
            :value="menu.description"
            type="text"
            :disabled="disabled"
            :placeholder="t('studio.assetWorkbench.menuDescriptionPlaceholder')"
            @input="
              emit('update:menu', {
                description: ($event.target as HTMLInputElement).value,
              })
            "
          />
        </label>
      </div>

      <div class="mf-grid">
        <label class="mf-field">
          <span>{{ t("studio.assetWorkbench.menuPosition") }}</span>
          <input
            :value="menu.uiPosition ?? ''"
            type="number"
            :disabled="disabled"
            :placeholder="t('studio.assetWorkbench.menuPositionPlaceholder')"
            @input="onPosition"
          />
        </label>
        <label class="mf-field">
          <span>{{ t("studio.assetWorkbench.menuCost") }}</span>
          <input
            :value="menu.constructionCost ?? ''"
            type="number"
            :disabled="disabled"
            :placeholder="t('studio.assetWorkbench.menuCostPlaceholder')"
            @input="onCost('constructionCost', $event)"
          />
        </label>
        <label class="mf-field">
          <span>{{ t("studio.assetWorkbench.menuUpkeep") }}</span>
          <input
            :value="menu.maintenanceCost ?? ''"
            type="number"
            :disabled="disabled"
            :placeholder="t('studio.assetWorkbench.menuUpkeepPlaceholder')"
            @input="onCost('maintenanceCost', $event)"
          />
        </label>
      </div>

      <div class="mf-grid">
        <div class="mf-field">
          <span>{{ t("studio.assetWorkbench.menuIcon") }}</span>
          <button
            type="button"
            class="mf-upload"
            :class="{ importing: importingImage, filled: menu.iconPng }"
            :disabled="disabled || importingImage || !project"
            @click="uploadMenuImage('icon')"
          >
            <img
              v-if="iconPreview"
              :src="iconPreview"
              :alt="t('studio.assetWorkbench.menuIconPreview')"
            />
            <span v-else class="mf-upload-empty">
              {{ t("studio.assetWorkbench.menuIconUpload") }}
            </span>
            <span
              v-if="menu.iconPng"
              class="mf-upload-clear"
              role="button"
              :title="t('studio.assetWorkbench.imageRemove')"
              @click.stop="emit('update:menu', { iconPng: '' })"
              >×</span
            >
          </button>
        </div>
        <div class="mf-field">
          <span>{{ t("studio.assetWorkbench.menuMarquee") }}</span>
          <button
            type="button"
            class="mf-upload marquee"
            :class="{ importing: importingImage, filled: menu.marqueePng }"
            :disabled="disabled || importingImage || !project"
            @click="uploadMenuImage('marquee')"
          >
            <img
              v-if="marqueePreview"
              :src="marqueePreview"
              :alt="t('studio.assetWorkbench.menuMarqueePreview')"
            />
            <span v-else class="mf-upload-empty">
              {{ t("studio.assetWorkbench.menuMarqueeUpload") }}
            </span>
            <span
              v-if="menu.marqueePng"
              class="mf-upload-clear"
              role="button"
              :title="t('studio.assetWorkbench.imageRemove')"
              @click.stop="emit('update:menu', { marqueePng: '' })"
              >×</span
            >
          </button>
        </div>
      </div>

      <section class="mf-entries">
        <header>
          <span>{{ t("studio.assetWorkbench.menuEntriesTitle") }}</span>
          <button type="button" :disabled="disabled" @click="addEntry">
            <FIcon name="Plus" :size="11" aria-label="" />
            {{ t("studio.assetWorkbench.menuAddEntry") }}
          </button>
        </header>
        <div v-for="entry in menu.entries" :key="entry.id" class="mf-entry">
          <label class="mf-field">
            <span>{{ t("studio.assetWorkbench.menuDimension") }}</span>
            <FSelect
              :model-value="entry.dimension"
              :disabled="disabled"
              :options="dimensionOptions"
              @update:model-value="onDimension(entry, String($event))"
            />
          </label>
          <label class="mf-field">
            <span>{{ t("studio.assetWorkbench.menuTargetCategory") }}</span>
            <FSelect
              :model-value="entry.categoryInstance"
              :disabled="disabled"
              :options="entryTargetOptions(entry.dimension)"
              :invalid="!entry.categoryInstance"
              @update:model-value="
                patchEntry(entry.id, { categoryInstance: String($event) })
              "
            />
          </label>
          <button
            type="button"
            class="mf-remove"
            :disabled="disabled"
            :title="t('studio.assetWorkbench.menuRemoveEntry')"
            @click="removeEntry(entry.id)"
          >
            <FIcon name="Trash2" :size="12" aria-label="" />
          </button>
        </div>
        <p v-if="!menu.entries.length" class="mf-note">
          {{ t("studio.assetWorkbench.menuIssue.menuEntryRequired") }}
        </p>
      </section>

      <ul v-if="issues.length" class="mf-issues">
        <li v-for="issue in issues" :key="issue">{{ issueLabel(issue) }}</li>
      </ul>
  </div>
</template>

<style scoped>
.menu-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.menu-form.compact {
  gap: 6px;
}
.mf-switch {
  align-items: center;
  display: flex;
  font-size: 12px;
  gap: 7px;
}
.mf-grid {
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.mf-field {
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 4px;
  min-width: 0;
}
.mf-field > span {
  color: var(--muted-foreground);
}
.mf-field input {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  min-height: 30px;
  min-width: 0;
  padding: 0 8px;
}
.mf-file {
  display: flex;
  gap: 5px;
}
.mf-file input {
  flex: 1;
}
.mf-file button {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  flex: none;
  font: inherit;
  font-size: 11px;
  padding: 0 8px;
}
.mf-field input.invalid {
  border-color: var(--danger, #e5484d);
}
.mf-field :deep(.f-select) {
  font-size: 12px;
  min-height: 30px;
}
.mf-entries {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.mf-entries > header {
  align-items: center;
  display: flex;
  font-size: 12px;
  font-weight: 600;
  justify-content: space-between;
}
.mf-entries > header button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 4px;
  min-height: 26px;
  padding: 0 8px;
}
.mf-entry {
  align-items: flex-end;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: grid;
  gap: 8px;
  grid-template-columns: 140px minmax(0, 1fr) 30px;
  padding: 8px;
}
.mf-remove {
  background: none;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  height: 30px;
  justify-content: center;
}
.mf-remove:hover {
  color: var(--danger, #e5484d);
}
.mf-note {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
}
.mf-issues {
  color: var(--danger, #e5484d);
  font-size: 11px;
  list-style: none;
  margin: 0;
  padding: 0;
}
</style>

<style scoped>
/* HBuilderX 风格图片预览（2026-11-07）：图标 72px 方形 + 背景图 16:9 */
.mf-upload {
  margin-top: 6px;
  width: 72px;
  height: 72px;
  border-radius: 6px;
  border: 1px dashed rgba(255, 255, 255, 0.25);
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(255, 255, 255, 0.04);
  cursor: pointer;
  padding: 0;
}
.mf-upload:hover:not(:disabled) {
  border-color: rgba(120, 190, 255, 0.6);
}
.mf-upload img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}
.mf-upload.marquee {
  width: 128px;
  height: 72px;
}
.mf-upload.marquee img {
  object-fit: cover;
}
.mf-upload-empty {
  font-size: 10px;
  color: var(--text-muted, rgba(255, 255, 255, 0.4));
  text-align: center;
  padding: 0 4px;
}
.mf-upload.importing {
  opacity: 0.55;
}
.mf-upload-clear {
  position: absolute;
  top: 0;
  right: 0;
  width: 18px;
  height: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  line-height: 1;
  border-radius: 0 6px 0 6px;
  background: rgba(220, 60, 60, 0.9);
  color: #fff;
  cursor: pointer;
}
.mf-upload-clear:hover {
  background: rgba(220, 60, 60, 1);
}
.mf-upload {
  position: relative;
}
</style>
