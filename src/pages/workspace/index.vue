<script setup lang="ts">
import { computed, onMounted, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import FDropdown from "@/components/ui/FDropdown.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FMarkdown from "@/components/markdown/FMarkdown.vue";
import { isTauri } from "@/api";
import { useToast } from "@/composables/useToast";
import { useWorkspace } from "@/composables/useWorkspace";
import DocumentStatusBar from "./components/DocumentStatusBar.vue";
import DocumentTabs from "./components/DocumentTabs.vue";
import MarkdownEditor from "./components/MarkdownEditor.vue";
import WorkspaceTree, { type TreeAction } from "./components/WorkspaceTree.vue";
import { tabTitle } from "./editor-tabs";
import { documentStats } from "./document-stats";
import { useDocumentTabs } from "./use-document-tabs";

/**
 * 文档工作区（2026-10-29 Obsidian 化重构）：去页面大标题，工作区
 * 撑满内容容器——左侧扁平文档树，右侧多页签编辑区（打开/切换/关闭、
 * 脏标记、localStorage 恢复）+ 格式工具栏 + 底部字数状态栏。
 * 页签与编辑变换的纯逻辑在 editor-tabs.ts / markdown-input.ts。
 */
const { t } = useI18n();
const toast = useToast();
const workspace = useWorkspace();
const { status, entries, error, isConfigured, legacyStatus, syncing } =
  workspace;
const tabs = useDocumentTabs({
  confirmCloseDirty: () =>
    window.confirm(
      t("workspace.tabDirtyConfirm", {
        name: tabTitle(tabs.active.value?.path ?? ""),
      }),
    ),
});
const { state: tabState, active: activeTab, savingPath } = tabs;

const isSaving = computed(
  () => savingPath.value !== "" && savingPath.value === activeTab.value?.path,
);
const stats = computed(() => documentStats(activeTab.value?.draft ?? ""));

const pending = shallowRef<TreeAction | null>(null);
const nameInput = shallowRef("");
const moveTarget = shallowRef("");
const moveMenuOpen = shallowRef(false);
function chooseMoveTarget(target: string) {
  moveTarget.value = target;
  moveMenuOpen.value = false;
}
const actionError = shallowRef("");
const moveTargets = computed(() => {
  if (!pending.value) return [] as string[];
  const current = pending.value.path;
  return [
    "",
    ...entries.value
      .filter((item) => item.kind === "folder")
      .map((item) => item.relativePath),
  ].filter(
    (candidate) =>
      !current ||
      (candidate !== current && !candidate.startsWith(`${current}/`)),
  );
});

onMounted(async () => {
  await workspace.loadStatus();
  if (workspace.isConfigured.value) await workspace.loadEntries();
  await workspace.loadLegacyStatus();
});
/** 历史目录一次性迁移（2026-10-29 目录治理）：拷入安装目录 docs/ 后
 * 切换配置；无待同步内容时按钮置灰（legacyStatus.canSync=false）。 */
async function syncLegacy() {
  const copied = await workspace.syncLegacy();
  if (copied > 0) {
    toast.success(t("workspace.synced", { count: copied }));
  }
}
function openFile(path: string) {
  tabs.open(path);
}
function handleAction(action: TreeAction) {
  actionError.value = "";
  const baseName = action.path.split("/").pop() ?? "";
  nameInput.value =
    action.type === "rename"
      ? baseName
      : action.type === "create-markdown"
        ? t("workspace.newMarkdownName")
        : "";
  moveTarget.value = action.isFile ? action.parent : "";
  pending.value = action;
}
function closeDialog() {
  pending.value = null;
  actionError.value = "";
}
function markdownName(value: string) {
  return value.toLowerCase().endsWith(".md") ? value : `${value}.md`;
}
function joinedPath(parent: string, name: string) {
  return parent ? `${parent}/${name}` : name;
}
async function confirmAction() {
  const action = pending.value;
  if (!action) return;
  const name = nameInput.value.trim();
  try {
    if (action.type === "create-folder") {
      if (!name) return;
      await workspace.createFolderIn(action.parent, name);
    } else if (action.type === "create-markdown") {
      if (!name) return;
      const path = joinedPath(action.parent, markdownName(name));
      await workspace.createMarkdownIn(action.parent, markdownName(name));
      openFile(path);
    } else if (action.type === "rename") {
      if (!name) return;
      const parent = action.path.includes("/")
        ? action.path.slice(0, action.path.lastIndexOf("/"))
        : "";
      await workspace.renameEntry(action.path, name);
      tabs.rebase(
        action.path,
        action.isFile
          ? joinedPath(parent, markdownName(name))
          : joinedPath(parent, name),
      );
    } else if (action.type === "move") {
      const name2 = action.path.split("/").pop() ?? "";
      await workspace.moveEntry(action.path, moveTarget.value);
      tabs.rebase(action.path, joinedPath(moveTarget.value, name2));
    }
    closeDialog();
  } catch (cause) {
    actionError.value = cause instanceof Error ? cause.message : String(cause);
  }
}
</script>

<template>
  <section class="workspace-page">
    <p v-if="!isTauri()" class="runtime-note" role="status">
      <FIcon name="Info" :size="16" aria-label="" />{{
        $t("runtime.browserNotice")
      }}
    </p>
    <article v-if="!isConfigured" class="setup-card">
      <div class="setup-icon" aria-hidden="true">
        <FIcon name="FolderPlus" :size="22" />
      </div>
      <div>
        <FTypography :header="3" spacing="none">{{
          $t("workspace.internalMissingTitle")
        }}</FTypography
        ><FTypography paragraphy type="secondary">{{
          $t("workspace.internalMissingDescription")
        }}</FTypography>
      </div>
      <p v-if="error" class="error-message" role="alert">{{ error }}</p>
    </article>
    <div v-else class="workspace-shell">
      <WorkspaceTree
        :entries="entries"
        :title="$t('workspace.folders')"
        :empty-hint="$t('workspace.empty')"
        :selected-path="tabState.activePath"
        @open="openFile"
        @action="handleAction"
      >
        <template #actions>
          <button
            v-if="legacyStatus?.legacyDir"
            class="sync-button"
            type="button"
            :disabled="!legacyStatus.canSync || syncing"
            :title="
              $t('workspace.syncHint', { count: legacyStatus.pendingCount })
            "
            @click="syncLegacy"
          >
            <FIcon
              :name="syncing ? 'Loader2' : 'FolderSync'"
              :size="14"
              aria-label=""
            />{{
              syncing
                ? $t("workspace.syncing")
                : $t("workspace.syncLegacy", {
                    count: legacyStatus.pendingCount,
                  })
            }}
          </button>
        </template>
      </WorkspaceTree>
      <section class="editor-pane">
        <DocumentTabs
          :tabs="tabState.tabs"
          :active-path="tabState.activePath"
          @activate="tabs.activate"
          @close="tabs.close"
        >
          <template #actions>
            <template v-if="activeTab">
              <div
                class="mode-toggle"
                role="group"
                :aria-label="$t('workspace.modeLabel')"
              >
                <button
                  type="button"
                  :class="{ on: activeTab.mode === 'edit' }"
                  @click="tabs.setMode(activeTab.path, 'edit')"
                >
                  <FIcon name="Pencil" :size="13" aria-label="" />{{
                    $t("workspace.edit")
                  }}
                </button>
                <button
                  type="button"
                  :class="{ on: activeTab.mode === 'preview' }"
                  @click="tabs.setMode(activeTab.path, 'preview')"
                >
                  <FIcon name="Eye" :size="13" aria-label="" />{{
                    $t("workspace.preview")
                  }}
                </button>
              </div>
              <button
                class="save-button"
                type="button"
                :disabled="!activeTab.dirty || isSaving"
                :title="$t('workspace.saveShortcut')"
                @click="tabs.save()"
              >
                <FIcon
                  :name="isSaving ? 'Loader2' : 'Save'"
                  :size="13"
                  aria-label=""
                />{{ $t("workspace.save") }}
              </button>
            </template>
          </template>
        </DocumentTabs>
        <div class="editor-body">
          <div v-if="!activeTab" class="editor-empty">
            <FIcon name="FileText" :size="26" aria-label="" />
            <FTypography :header="3">{{
              $t("workspace.emptyTitle")
            }}</FTypography>
            <FTypography paragraphy type="secondary">{{
              $t("workspace.emptyHint")
            }}</FTypography>
          </div>
          <template v-else>
            <p v-if="activeTab.error" class="error-message" role="alert">
              {{ activeTab.error }}
            </p>
            <FSpinner
              v-if="activeTab.loading"
              size="sm"
              :label="$t('common.loading')"
            />
            <MarkdownEditor
              v-else-if="activeTab.mode === 'edit'"
              :draft="activeTab.draft"
              @update:draft="tabs.setDraft(activeTab.path, $event)"
              @save="tabs.save()"
            />
            <div v-else class="editor-preview">
              <FMarkdown :source="activeTab.draft" />
            </div>
          </template>
        </div>
        <DocumentStatusBar
          :path="activeTab?.path ?? ''"
          :root-path="status?.rootPath ?? ''"
          :words="stats.words"
          :characters="stats.characters"
          :dirty="activeTab?.dirty ?? false"
        />
      </section>
    </div>
    <Teleport to="body">
      <div v-if="pending" class="dialog-overlay" @mousedown.self="closeDialog">
        <div
          class="dialog"
          role="dialog"
          aria-modal="true"
          :aria-label="
            $t(
              `workspace.dialog${pending.type === 'create-folder' ? 'CreateFolder' : pending.type === 'create-markdown' ? 'CreateMarkdown' : pending.type === 'rename' ? 'Rename' : 'Move'}`,
            )
          "
        >
          <FTypography :header="4" spacing="none">{{
            pending.type === "create-folder"
              ? $t("workspace.dialogCreateFolder")
              : pending.type === "create-markdown"
                ? $t("workspace.dialogCreateMarkdown")
                : pending.type === "rename"
                  ? $t("workspace.dialogRename")
                  : $t("workspace.dialogMove")
          }}</FTypography>
          <p v-if="pending.path" class="dialog-target">{{ pending.path }}</p>
          <label v-if="pending.type !== 'move'" for="tree-action-name">{{
            $t("workspace.nameLabel")
          }}</label>
          <input
            v-if="pending.type !== 'move'"
            id="tree-action-name"
            v-model="nameInput"
            autocomplete="off"
            @keydown.enter.prevent="confirmAction"
          />
          <template v-else>
            <span class="select-label">{{ $t("workspace.targetLabel") }}</span>
            <FDropdown v-model:open="moveMenuOpen" :width="280">
              <template #trigger>
                <button
                  id="tree-action-target"
                  type="button"
                  class="target-trigger"
                >
                  <span class="target-text">{{
                    moveTarget || $t("workspace.targetRoot")
                  }}</span>
                  <FIcon name="ChevronDown" :size="12" aria-label="" />
                </button>
              </template>
              <button type="button" @click="chooseMoveTarget('')">
                <FIcon
                  :name="!moveTarget ? 'Check' : 'Folder'"
                  :size="14"
                  aria-label=""
                />
                {{ $t("workspace.targetRoot") }}
              </button>
              <button
                v-for="target in moveTargets.filter(Boolean)"
                :key="target"
                type="button"
                @click="chooseMoveTarget(target)"
              >
                <FIcon
                  :name="moveTarget === target ? 'Check' : 'FolderOpen'"
                  :size="14"
                  aria-label=""
                />
                {{ target }}
              </button>
            </FDropdown>
          </template>
          <p v-if="actionError" class="error-message" role="alert">
            {{ actionError }}
          </p>
          <div class="dialog-actions">
            <button class="secondary-button" type="button" @click="closeDialog">
              {{ $t("workspace.cancel") }}
            </button>
            <button class="primary-button" type="button" @click="confirmAction">
              {{ $t("workspace.confirm") }}
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
/* 撑满内容容器（配合路由 meta.fullbleed：外层去内边距、隐藏页脚） */
.workspace-page {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
}
.runtime-note {
  align-items: center;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  color: var(--muted-foreground);
  display: flex;
  flex: none;
  font-size: 12px;
  gap: 8px;
  padding: 8px 14px;
}
.setup-card {
  align-items: start;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  display: grid;
  gap: 12px;
  grid-template-columns: auto 1fr;
  margin: auto;
  max-width: 760px;
  padding: 24px;
}
.setup-icon {
  align-items: center;
  background: var(--accent);
  border-radius: var(--radius-md);
  color: var(--primary);
  display: flex;
  height: 42px;
  justify-content: center;
  width: 42px;
}
.workspace-shell {
  display: grid;
  flex: 1;
  grid-template-columns: minmax(200px, 260px) minmax(0, 1fr);
  min-height: 0;
}
.editor-pane {
  background: var(--background);
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
}
.editor-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  overflow: auto;
  scrollbar-width: thin;
}
.editor-empty {
  align-items: center;
  color: var(--subtle-foreground);
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
  margin: auto;
  padding: 24px;
  text-align: center;
}
.editor-preview {
  margin: 0 auto;
  padding: 18px 18px 40px;
  width: min(100%, 880px);
}
.error-message {
  color: var(--danger);
  font-size: 12px;
  margin: 10px 14px 0;
}
/* 页签条右侧动作：编辑/预览切换 + 保存 */
.mode-toggle {
  align-items: center;
  background: var(--surface-hover);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: inline-flex;
  overflow: hidden;
}
.mode-toggle button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11.5px;
  font-weight: 650;
  gap: 5px;
  min-height: 24px;
  padding: 0 9px;
}
.mode-toggle button.on {
  background: var(--surface-elevated);
  color: var(--foreground);
}
.save-button {
  align-items: center;
  background: var(--primary);
  border: 1px solid var(--primary);
  border-radius: var(--radius-sm);
  color: var(--primary-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11.5px;
  font-weight: 700;
  gap: 5px;
  min-height: 26px;
  padding: 0 10px;
  white-space: nowrap;
}
.save-button:disabled {
  cursor: default;
  opacity: 0.45;
}
/* 树面板头部的同步按钮（小尺寸紧凑形态） */
.sync-button {
  align-items: center;
  background: var(--surface-hover);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 10.5px;
  font-weight: 650;
  gap: 4px;
  min-height: 22px;
  padding: 0 7px;
  white-space: nowrap;
}
.sync-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.sync-button:not(:disabled):hover {
  background: var(--accent);
  color: var(--foreground);
}
.dialog-overlay {
  align-items: center;
  background: oklch(0.1 0.01 260 / 0.45);
  display: flex;
  inset: 0;
  justify-content: center;
  position: fixed;
  z-index: 70;
}
.dialog {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
  display: grid;
  gap: 12px;
  padding: 20px;
  width: min(400px, calc(100vw - 32px));
}
.dialog-target {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: -6px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dialog label {
  color: var(--muted-foreground);
  font-size: 12px;
}
.dialog input,
.dialog select {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  min-width: 0;
  padding: 9px 10px;
  width: 100%;
}
.dialog input:focus-visible,
.dialog select:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.dialog-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.primary-button,
.secondary-button {
  align-items: center;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  font-weight: 700;
  gap: 7px;
  justify-content: center;
  min-height: 36px;
  padding: 0 12px;
  white-space: nowrap;
}
.primary-button {
  background: var(--primary);
  border: 1px solid var(--primary);
  color: var(--primary-foreground);
}
.secondary-button {
  background: var(--surface-hover);
  border: 1px solid var(--border);
  color: var(--foreground);
}
.target-trigger {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 8px;
  justify-content: space-between;
  min-height: 34px;
  padding: 4px 10px;
  width: 100%;
}
.target-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.select-label {
  color: var(--muted-foreground);
  font-size: 12px;
}
@media (max-width: 780px) {
  .workspace-shell {
    grid-template-columns: 1fr;
    grid-template-rows: minmax(150px, 32%) minmax(0, 1fr);
  }
  .workspace-tree {
    border-inline-end: 0;
    border-bottom: 1px solid var(--border);
  }
  .setup-card {
    grid-template-columns: auto 1fr;
    margin: 16px auto;
  }
}
</style>
