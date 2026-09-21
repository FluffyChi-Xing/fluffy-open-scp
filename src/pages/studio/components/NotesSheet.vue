<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FSheet from "@/components/ui/FSheet.vue";
import FMarkdown from "@/components/markdown/FMarkdown.vue";
import { isTauri, tauriApi } from "@/api";
import { useToast } from "@/composables/useToast";
import type { MarkdownDocument, ModProjectView, WorkspaceEntry } from "@/api/tauri";

/**
 * 模组笔记（全屏 Code 工作台 toolbar 的二级 sheet）：
 * 笔记保存在「文档工作区」根目录的 mods/<模组名>/ 下（与项目解耦，
 * 换 modRoot 不丢笔记）。复用 workspace 的 markdown 命令：
 * createFolder 建目录 → createMarkdown 建笔记 → read/write 读写
 * （sha256 乐观锁）。支持编辑与 FMarkdown 预览切换。
 */
const props = defineProps<{ project: ModProjectView }>();
const open = defineModel<boolean>("open", { default: false });

const { t } = useI18n();
const toast = useToast();

/** 文档工作区内本模组的笔记目录前缀。 */
const notesPrefix = computed(() => `mods/${props.project.name}/`);

const notes = ref<WorkspaceEntry[]>([]);
const loading = ref(false);
const newTitle = ref("");
const active = ref<MarkdownDocument | null>(null);
const draft = ref("");
const preview = ref(false);
const saving = ref(false);

watch(open, (value) => {
  if (value && isTauri()) void ensureDirAndLoad();
});

async function ensureDirAndLoad() {
  // 目录缺失时创建（create_folder 对已存在目录幂等）。
  try {
    await tauriApi.workspace.createFolder(`mods/${props.project.name}`);
  } catch {
    /* 已存在或父级存在时忽略，列表阶段自然兜底 */
  }
  await loadNotes();
}

async function loadNotes() {
  loading.value = true;
  try {
    const entries = await tauriApi.workspace.list();
    notes.value = entries.filter(
      (entry) =>
        entry.kind === "file" &&
        entry.relativePath.startsWith(notesPrefix.value),
    );
  } catch {
    toast.error(t("studio.code.notes.loadFailed"));
  } finally {
    loading.value = false;
  }
}

async function createNote() {
  const title = newTitle.value.trim();
  if (!title) return;
  const relative = `${notesPrefix.value}${title}.md`;
  try {
    await tauriApi.workspace.createMarkdown(
      relative,
      `# ${title}\n\n`,
    );
    newTitle.value = "";
    await loadNotes();
    active.value = {
      relativePath: relative,
      content: `# ${title}\n\n`,
      size: title.length + 6,
      revision: "",
    };
    draft.value = active.value.content;
    preview.value = false;
  } catch (cause) {
    const code = (cause as { code?: string }).code ?? "";
    toast.error(
      code === "already_exists"
        ? t("studio.code.notes.alreadyExists")
        : t("studio.code.notes.loadFailed"),
    );
  }
}

function noteTitle(relativePath: string): string {
  const name = relativePath.slice(notesPrefix.value.length);
  return name.replace(/\.md$/i, "");
}

async function selectNote(entry: WorkspaceEntry) {
  try {
    const document = await tauriApi.workspace.readMarkdown(entry.relativePath);
    active.value = document;
    draft.value = document.content;
    preview.value = false;
  } catch {
    toast.error(t("studio.code.notes.loadFailed"));
  }
}

async function saveNote() {
  const document = active.value;
  if (!document || saving.value) return;
  saving.value = true;
  try {
    const saved = await tauriApi.workspace.writeMarkdown(
      document.relativePath,
      draft.value,
      document.revision || undefined,
    );
    active.value = saved;
    toast.success(t("studio.code.notes.saved"));
  } catch (cause) {
    const code = (cause as { code?: string }).code ?? "";
    toast.error(
      code === "revision_conflict"
        ? t("studio.code.notes.conflict")
        : t("studio.code.notes.loadFailed"),
    );
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <FSheet v-model:open="open" width="min(680px, 94vw)" :label="$t('studio.code.notes.title')">
    <div class="notes">
      <header class="notes-head">
        <div>
          <p class="eyebrow">{{ $t("studio.code.sheetTitle") }}</p>
          <FTypography :header="2" spacing="none">{{
            $t("studio.code.notes.title")
          }}</FTypography>
          <p class="notes-path">{{ notesPrefix }}</p>
        </div>
        <button
          class="icon-close"
          type="button"
          :aria-label="$t('common.close')"
          @click="open = false"
        >
          <FIcon name="X" :size="15" aria-label="" />
        </button>
      </header>

      <div class="note-create">
        <input
          v-model="newTitle"
          class="note-input"
          type="text"
          :placeholder="$t('studio.code.notes.newPlaceholder')"
          @keydown.enter="createNote"
        />
        <button class="note-create-button" type="button" @click="createNote">
          <FIcon name="Plus" :size="13" aria-label="" />
          {{ $t("studio.code.notes.create") }}
        </button>
      </div>

      <div v-if="loading && !notes.length" class="notes-empty">
        {{ $t("common.loading") }}
      </div>
      <div v-else-if="!notes.length" class="notes-empty">
        {{ $t("studio.code.notes.empty") }}
      </div>
      <ul v-else class="notes-list">
        <li v-for="entry in notes" :key="entry.relativePath">
          <button
            type="button"
            class="note-row"
            :class="{ active: active?.relativePath === entry.relativePath }"
            @click="selectNote(entry)"
          >
            <FIcon name="FileText" :size="13" aria-label="" />
            {{ noteTitle(entry.relativePath) }}
          </button>
        </li>
      </ul>

      <template v-if="active">
        <div class="note-toolbar">
          <span class="note-active-path">{{ active.relativePath }}</span>
          <button
            class="note-tool"
            type="button"
            :class="{ active: !preview }"
            @click="preview = false"
          >
            {{ $t("studio.code.notes.edit") }}
          </button>
          <button
            class="note-tool"
            type="button"
            :class="{ active: preview }"
            @click="preview = true"
          >
            {{ $t("studio.code.notes.preview") }}
          </button>
          <button
            class="note-tool primary"
            type="button"
            :disabled="saving || preview"
            @click="saveNote"
          >
            <FIcon name="Save" :size="12" aria-label="" />
            {{ $t("studio.code.notes.save") }}
          </button>
        </div>
        <textarea
          v-if="!preview"
          v-model="draft"
          class="note-editor"
          spellcheck="false"
        ></textarea>
        <div v-else class="note-preview">
          <FMarkdown :source="draft" />
        </div>
      </template>
    </div>
  </FSheet>
</template>

<style scoped>
.notes {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  padding: 16px;
}
.notes-head {
  align-items: flex-start;
  display: flex;
  justify-content: space-between;
}
.notes-head :deep(h1),
.notes-head :deep(h2) {
  margin: 0;
}
.notes-path {
  color: var(--subtle-foreground);
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  margin: 4px 0 0;
}
.icon-close {
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  min-height: 28px;
  padding: 0 6px;
}
.icon-close:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.note-create {
  display: flex;
  gap: 8px;
}
.note-input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  flex: 1;
  font: inherit;
  font-size: 12.5px;
  min-height: 30px;
  padding: 0 10px;
}
.note-input:focus {
  outline: 1px solid var(--primary);
}
.note-create-button {
  align-items: center;
  background: var(--primary);
  border: 1px solid var(--primary);
  border-radius: var(--radius-sm);
  color: #fff;
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  gap: 6px;
  min-height: 30px;
  padding: 0 12px;
}
.notes-empty {
  color: var(--subtle-foreground);
  font-size: 12px;
  padding: 12px 0;
}
.notes-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  list-style: none;
  margin: 0;
  max-height: 180px;
  overflow: auto;
  padding: 0;
}
.note-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 8px;
  min-height: 28px;
  padding: 0 8px;
  text-align: start;
  width: 100%;
}
.note-row:hover {
  background: var(--surface-hover);
}
.note-row.active {
  color: var(--foreground);
  font-weight: 650;
}
.note-toolbar {
  align-items: center;
  border-top: 1px solid var(--border);
  display: flex;
  gap: 6px;
  padding-top: 10px;
}
.note-active-path {
  color: var(--subtle-foreground);
  flex: 1;
  font-family: var(--font-mono, monospace);
  font-size: 10.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.note-tool {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11.5px;
  min-height: 26px;
  padding: 0 10px;
}
.note-tool.active {
  color: var(--foreground);
}
.note-tool.primary {
  background: var(--primary);
  border-color: var(--primary);
  color: #fff;
}
.note-tool:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.note-editor {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  flex: 1;
  font-family: var(--font-mono, monospace);
  font-size: 12px;
  line-height: 1.6;
  min-height: 220px;
  padding: 10px;
  resize: none;
  width: 100%;
}
.note-preview {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  flex: 1;
  min-height: 220px;
  overflow: auto;
  padding: 12px;
}
</style>
