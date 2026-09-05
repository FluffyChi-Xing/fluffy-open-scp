<script setup lang="ts">
import { computed, onMounted, shallowRef } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import { isTauri } from "@/api";
import type { FTreeNode } from "@/components/extensions/tree";
import WorkspaceDocument from "./components/WorkspaceDocument.vue";
import WorkspaceTree from "./components/WorkspaceTree.vue";
import { useWorkspace } from "@/composables/useWorkspace";

const workspace = useWorkspace();
const { status, folders, document, saving, error, isConfigured } = workspace;
const rootInput = shallowRef("");
const folderInput = shallowRef("mods/example");
const tree = computed(() => folderTree(folders.value));

onMounted(async () => {
  if (!isTauri()) return;
  await workspace.loadStatus();
  if (workspace.isConfigured.value) await workspace.loadFolders();
});
async function configureRoot() {
  if (!rootInput.value.trim()) return;
  await workspace.setRoot(rootInput.value.trim());
  await workspace.loadFolders();
}
async function createFolder() {
  if (!folderInput.value.trim()) return;
  await workspace.createFolder(folderInput.value.trim());
}
async function selectFolder(keys: string[]) {
  const path = keys[0];
  if (!path) return;
  const folder = folders.value.find((item) => item.relativePath === path);
  await workspace.select(folder?.readmeRelativePath ?? `${path}/README.md`);
}
function folderTree(folders: readonly { relativePath: string }[]): FTreeNode[] {
  const roots: FTreeNode[] = [];
  for (const folder of folders) {
    const parts = folder.relativePath.split("/");
    let level = roots;
    let path = "";
    for (const part of parts) {
      path = path ? `${path}/${part}` : part;
      let node = level.find((item) => item.key === path);
      if (!node) {
        node = { key: path, label: part, icon: "FolderOpen", children: [] };
        level.push(node);
      }
      level = node.children ?? (node.children = []);
    }
  }
  return roots;
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
      <div class="heading-mark" aria-hidden="true">
        <FIcon name="BookOpen" :size="22" />
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
          /><button class="primary-button" type="submit">
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
        <form class="folder-form" @submit.prevent="createFolder">
          <label for="new-folder">{{ $t("workspace.newFolder") }}</label>
          <div class="form-row">
            <input
              id="new-folder"
              v-model="folderInput"
              :placeholder="$t('workspace.folderPlaceholder')"
              autocomplete="off"
            /><button class="secondary-button" type="submit">
              <FIcon name="Plus" :size="16" aria-label="" />{{
                $t("workspace.createFolder")
              }}
            </button>
          </div>
        </form>
      </div>
      <div class="workspace-grid">
        <WorkspaceTree
          :nodes="tree"
          :title="$t('workspace.folders')"
          :empty-title="$t('workspace.empty')"
          @select="selectFolder"
        /><WorkspaceDocument
          :document="document"
          :saving="saving"
          :error="error"
          :select-title="$t('workspace.selectTitle')"
          :select-description="$t('workspace.selectDescription')"
          @save="workspace.save"
        /></div
    ></template>
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
.heading-mark {
  align-items: center;
  background: var(--accent);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  color: var(--primary);
  display: flex;
  height: 52px;
  justify-content: center;
  width: 52px;
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
.root-form,
.folder-form {
  grid-column: 1/-1;
}
.root-form label,
.folder-form label {
  color: var(--muted-foreground);
  display: block;
  font-size: 12px;
  margin-bottom: 7px;
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
