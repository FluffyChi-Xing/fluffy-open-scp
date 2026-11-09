<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import { isTauri, tauriApi } from "@/api";
import { extensionIconUrl } from "@/lib/resource-types";
import {
  flattenCodeTree,
  formatCodeSize,
  type CodeTreeNodeDto,
} from "../code-support";
import type { CodeTreeResponse } from "@/api/tauri";

/**
 * GitHub 风格文件树（只读）：首层展开，文件夹可折叠。
 * 详情 Code tab 中央区使用；完整编辑/预览在 Develop tab。
 */
const props = defineProps<{ projectRelPath: string }>();

const tree = shallowRef<CodeTreeResponse | null>(null);
const loading = ref(false);
const error = ref("");
const expanded = ref(new Set<string>());

const rows = computed(() =>
  flattenCodeTree(tree.value?.entries ?? [], expanded.value),
);

async function loadTree() {
  if (!isTauri() || !props.projectRelPath) return;
  loading.value = true;
  error.value = "";
  try {
    const response = await tauriApi.workspace.codeTree(props.projectRelPath);
    tree.value = response;
    expanded.value = new Set(
      response.entries
        .filter((entry) => entry.kind === "folder")
        .map((entry) => entry.relativePath),
    );
  } catch (cause) {
    error.value =
      cause && typeof cause === "object" && "message" in cause
        ? String((cause as { message: unknown }).message)
        : String(cause);
    tree.value = null;
  } finally {
    loading.value = false;
  }
}

function toggle(node: CodeTreeNodeDto | { relativePath: string; kind: string }) {
  if (node.kind !== "folder") return;
  const next = new Set(expanded.value);
  if (next.has(node.relativePath)) next.delete(node.relativePath);
  else next.add(node.relativePath);
  expanded.value = next;
}

function iconFor(name: string, kind: string): string | undefined {
  if (kind === "folder") return undefined;
  return extensionIconUrl(name);
}

watch(
  () => props.projectRelPath,
  () => {
    void loadTree();
  },
  { immediate: true },
);
</script>

<template>
  <div class="file-tree">
    <div class="tree-toolbar">
      <span class="branch-pill">
        <FIcon name="GitBranch" :size="12" aria-label="" />
        main
      </span>
      <span v-if="tree" class="tree-meta">
        {{
          $t("studio.repo.treeStats", {
            files: tree.fileCount,
            folders: tree.folderCount,
          })
        }}
      </span>
      <button
        class="refresh"
        type="button"
        :disabled="loading"
        @click="loadTree"
      >
        <FIcon
          :name="loading ? 'Loader2' : 'RefreshCw'"
          :size="12"
          aria-label=""
        />
      </button>
    </div>

    <p v-if="error" class="tree-empty" role="alert">{{ error }}</p>
    <p v-else-if="loading && !tree" class="tree-empty">
      {{ $t("common.loading") }}
    </p>
    <p v-else-if="tree && !rows.length" class="tree-empty">
      {{ $t("studio.repo.treeEmpty") }}
    </p>
    <div v-else class="tree-table" role="table">
      <button
        v-for="row in rows"
        :key="row.relativePath"
        class="tree-row"
        type="button"
        :style="{ paddingLeft: `${10 + row.depth * 14}px` }"
        @click="toggle(row)"
      >
        <FIcon
          v-if="row.kind === 'folder'"
          :name="expanded.has(row.relativePath) ? 'FolderOpen' : 'Folder'"
          :size="14"
          aria-label=""
        />
        <img
          v-else-if="iconFor(row.name, row.kind)"
          class="file-icon"
          :src="iconFor(row.name, row.kind)!"
          alt=""
        />
        <FIcon v-else name="File" :size="14" aria-label="" />
        <span class="file-name">{{ row.name }}</span>
        <span class="file-size">{{
          row.size != null ? formatCodeSize(row.size) : ""
        }}</span>
      </button>
      <p v-if="tree?.truncated" class="tree-truncated">
        {{ $t("studio.repo.treeTruncated") }}
      </p>
    </div>
  </div>
</template>

<style scoped>
.file-tree {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  display: grid;
  gap: 0;
  min-width: 0;
  overflow: hidden;
}
.tree-toolbar {
  align-items: center;
  background: color-mix(in srgb, var(--surface-elevated) 70%, transparent);
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 10px;
  min-height: 40px;
  padding: 0 12px;
}
.branch-pill {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--foreground);
  display: inline-flex;
  font-size: 12px;
  gap: 5px;
  padding: 3px 10px;
}
.tree-meta {
  color: var(--muted-foreground);
  flex: 1;
  font-size: 11.5px;
}
.refresh {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  padding: 4px;
}
.refresh:hover:not(:disabled) {
  color: var(--foreground);
}
.tree-empty {
  color: var(--muted-foreground);
  font-size: 12.5px;
  margin: 0;
  padding: 28px 14px;
  text-align: center;
}
.tree-table {
  display: grid;
}
.tree-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  color: var(--foreground);
  cursor: default;
  display: grid;
  font: inherit;
  font-size: 13px;
  gap: 8px;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  min-height: 36px;
  padding-right: 12px;
  text-align: start;
}
.tree-row:hover {
  background: color-mix(in srgb, var(--surface-hover) 80%, transparent);
}
.file-icon {
  height: 14px;
  width: 14px;
}
.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-size {
  color: var(--subtle-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.tree-truncated {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 0;
  padding: 8px 12px 10px;
}
</style>
