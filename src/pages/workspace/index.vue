<script setup lang="ts">
import { computed, onMounted, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import { isTauri, tauriApi } from "@/api";
import WorkspaceDocument from "./components/WorkspaceDocument.vue";
import WorkspaceTree, { type TreeAction } from "./components/WorkspaceTree.vue";
import { useWorkspace } from "@/composables/useWorkspace";

const { t } = useI18n();
const workspace = useWorkspace();
const { status, entries, selectedPath, document, saving, error, isConfigured } =
  workspace;
const rootInput = shallowRef("");
const pending = shallowRef<TreeAction | null>(null);
const nameInput = shallowRef("");
const moveTarget = shallowRef("");
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
});
async function configureRoot() {
  if (!rootInput.value.trim()) return;
  await workspace.setRoot(rootInput.value.trim());
  await workspace.loadEntries();
}
async function chooseRoot() {
  if (!isTauri()) return;
  const path = await tauriApi.workspace.pickDirectory();
  if (!path) return;
  rootInput.value = path;
  await configureRoot();
}
function openFile(path: string) {
  void workspace.select(path);
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
      await workspace.createMarkdownIn(action.parent, markdownName(name));
    } else if (action.type === "rename") {
      if (!name) return;
      await workspace.renameEntry(action.path, name);
      if (action.isFile) {
        const parent = action.path.includes("/")
          ? action.path.slice(0, action.path.lastIndexOf("/"))
          : "";
        await workspace.select(
          `${parent ? `${parent}/` : ""}${markdownName(name)}`,
        );
      }
    } else if (action.type === "move") {
      await workspace.moveEntry(action.path, moveTarget.value);
    }
    closeDialog();
  } catch (cause) {
    actionError.value = cause instanceof Error ? cause.message : String(cause);
  }
}
</script>

<template>
  <section class="workspace-page">
    <header class="page-heading">
      <div>
        <p class="eyebrow">{{ $t("workspace.eyebrow") }}</p>
        <FTypography :header="1" spacing="none">{{
          $t("workspace.title")
        }}</FTypography
        ><FTypography paragraphy type="secondary">{{
          $t("workspace.description")
        }}</FTypography>
      </div>
    </header>
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
          $t("workspace.chooseTitle")
        }}</FTypography
        ><FTypography paragraphy type="secondary">{{
          $t("workspace.chooseDescription")
        }}</FTypography>
      </div>
      <form class="root-form" @submit.prevent="configureRoot">
        <label for="workspace-root">{{ $t("workspace.rootLabel") }}</label>
        <div class="form-row">
          <input
            id="workspace-root"
            v-model="rootInput"
            :placeholder="$t('workspace.rootPlaceholder')"
            autocomplete="off"
          /><button
            v-if="isTauri()"
            class="secondary-button"
            type="button"
            @click="chooseRoot"
          >
            <FIcon name="FolderOpen" :size="16" aria-label="" />{{
              $t("workspace.chooseFolder")
            }}</button
          ><button class="primary-button" type="submit">
            <FIcon name="FolderOpen" :size="16" aria-label="" />{{
              $t("workspace.openRoot")
            }}
          </button>
        </div>
      </form>
      <p v-if="error" class="error-message" role="alert">{{ error }}</p>
    </article>
    <template v-else
      ><div class="workspace-toolbar">
        <div>
          <FTypography :header="3" spacing="none">{{
            $t("workspace.rootLabel")
          }}</FTypography
          ><FTypography paragraphy type="secondary" spacing="none">{{
            status?.rootPath
          }}</FTypography>
        </div>
        <p class="toolbar-hint">{{ $t("workspace.contextHint") }}</p>
      </div>
      <div class="workspace-grid">
        <WorkspaceTree
          :entries="entries"
          :title="$t('workspace.folders')"
          :empty-hint="$t('workspace.empty')"
          :selected-path="selectedPath"
          @open="openFile"
          @action="handleAction"
        /><WorkspaceDocument
          :document="document"
          :saving="saving"
          :error="error"
          :select-title="$t('workspace.selectTitle')"
          :select-description="$t('workspace.selectDescription')"
          @save="workspace.save"
        /></div
    ></template>
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
            <label for="tree-action-target">{{
              $t("workspace.targetLabel")
            }}</label>
            <select id="tree-action-target" v-model="moveTarget">
              <option value="">{{ $t("workspace.targetRoot") }}</option>
              <option
                v-for="target in moveTargets.filter(Boolean)"
                :key="target"
                :value="target"
              >
                {{ target }}
              </option>
            </select>
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
.workspace-page {
  display: grid;
  gap: 24px;
}
.page-heading {
  align-items: flex-start;
  display: flex;
  justify-content: space-between;
}
.eyebrow {
  color: var(--primary);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.08em;
  margin: 0 0 10px;
  text-transform: uppercase;
}
.runtime-note,
.setup-card,
.workspace-toolbar {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
}
.runtime-note {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 12px;
  gap: 8px;
  padding: 12px 14px;
}
.setup-card {
  display: grid;
  gap: 12px;
  grid-template-columns: auto 1fr;
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
.root-form {
  grid-column: 1/-1;
}
.root-form label {
  color: var(--muted-foreground);
  display: block;
  font-size: 12px;
  margin-bottom: 7px;
}
.toolbar-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 0;
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
.form-row {
  display: flex;
  gap: 8px;
}
.form-row input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  min-width: 0;
  padding: 10px 11px;
  width: 100%;
}
.form-row input:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
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
.error-message {
  color: var(--danger);
  font-size: 12px;
  grid-column: 1/-1;
  margin: 0;
}
.workspace-toolbar {
  align-items: end;
  display: flex;
  gap: 24px;
  justify-content: space-between;
  padding: 18px 20px;
}
.folder-form {
  max-width: 430px;
}
.workspace-grid {
  display: grid;
  gap: 14px;
  grid-template-columns: minmax(210px, 0.35fr) minmax(0, 1fr);
  min-height: 500px;
}
@media (max-width: 780px) {
  .workspace-toolbar {
    align-items: stretch;
    flex-direction: column;
    gap: 16px;
  }
  .folder-form {
    max-width: none;
  }
  .workspace-grid {
    grid-template-columns: 1fr;
  }
  .setup-card {
    grid-template-columns: auto 1fr;
  }
  .form-row {
    flex-direction: column;
  }
  .form-row button {
    width: 100%;
  }
}
</style>
