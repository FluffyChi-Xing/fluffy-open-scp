<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import { isTauri, tauriApi } from "@/api";
import { command } from "@/api/tauri";
import type { CodeTreeNode } from "@/api/tauri";

/**
 * 工作台左栏：项目文件夹树（Blender 资产浏览口径）。
 * 点击 .glb/.lotm 直接进视口；.package 交给工作台解析 Lot property；
 * 头部导入按钮把 OS 文件复制进项目 assets/models|textures 后自动打开。
 */
const props = defineProps<{
  project: string;
  /** 当前激活的模型资产相对路径（高亮）。 */
  activeAsset: string;
}>();
const emit = defineEmits<{
  "open-asset": [relativePath: string];
  "open-package": [relativePath: string];
  imported: [];
}>();

const { t } = useI18n();
const tree = ref<CodeTreeNode[] | null>(null);
const loading = ref(false);
const error = ref("");
const expanded = ref(new Set<string>());
const importing = ref(false);

interface TreeRow {
  name: string;
  relativePath: string;
  depth: number;
  isFolder: boolean;
  size: number | null;
}

const rows = computed<TreeRow[]>(() => {
  const result: TreeRow[] = [];
  const walk = (nodes: CodeTreeNode[], depth: number) => {
    for (const node of [...nodes].sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === "folder" ? -1 : 1;
      return a.name.localeCompare(b.name);
    })) {
      result.push({
        name: node.name,
        relativePath: node.relativePath,
        depth,
        isFolder: node.kind === "folder",
        size: node.size,
      });
      if (node.kind === "folder" && expanded.value.has(node.relativePath))
        walk(node.children ?? [], depth + 1);
    }
  };
  walk(tree.value ?? [], 0);
  return result;
});

async function loadTree() {
  if (!isTauri()) return;
  loading.value = true;
  error.value = "";
  try {
    const response = await tauriApi.workspace.codeTree(props.project);
    tree.value = response.entries;
    if (!expanded.value.size) {
      // 首层目录默认展开（assets 优先可见）
      expanded.value = new Set(
        response.entries
          .filter((node) => node.kind === "folder")
          .map((node) => node.relativePath),
      );
    }
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}

function toggleFolder(row: TreeRow) {
  const next = new Set(expanded.value);
  if (next.has(row.relativePath)) next.delete(row.relativePath);
  else next.add(row.relativePath);
  expanded.value = next;
}

const MODEL_EXTENSIONS = [".glb", ".lotm"];

function onRowClick(row: TreeRow) {
  if (row.isFolder) {
    toggleFolder(row);
    return;
  }
  const lower = row.name.toLowerCase();
  if (MODEL_EXTENSIONS.some((extension) => lower.endsWith(extension))) {
    emit("open-asset", row.relativePath);
  } else if (lower.endsWith(".package")) {
    emit("open-package", row.relativePath);
  }
}

function fileIcon(name: string): string {
  const lower = name.toLowerCase();
  if (lower.endsWith(".glb") || lower.endsWith(".lotm")) return "Box";
  if (lower.endsWith(".package")) return "Package";
  if (["png", "jpg", "jpeg", "dds"].some((e) => lower.endsWith(`.${e}`)))
    return "Image";
  if (lower.endsWith(".json")) return "Braces";
  return "File";
}

/** 导入 OS 文件 → 项目 assets/（复制）→ 自动打开模型。 */
async function importFile() {
  const path = await open({
    multiple: false,
    filters: [
      {
        name: t("studio.assetWorkbench.importFilterName"),
        extensions: ["glb", "lotm", "package", "png", "jpg", "jpeg", "dds", "json"],
      },
    ],
  });
  if (typeof path !== "string") return;
  importing.value = true;
  try {
    const value = await command<{ asset: string }>("code_flow", {
      request: { project: props.project, action: "import-asset", payload: { path } },
    });
    await loadTree();
    const lower = value.asset.toLowerCase();
    if (MODEL_EXTENSIONS.some((extension) => lower.endsWith(extension))) {
      emit("open-asset", value.asset);
    }
    emit("imported");
  } catch (cause) {
    error.value = String(cause);
  } finally {
    importing.value = false;
  }
}

defineExpose({ refresh: loadTree });
onMounted(loadTree);
</script>

<template>
  <div class="asset-tree" :aria-label="t('studio.assetWorkbench.tree')">
    <header class="tree-head">
      <strong>{{ t("studio.assetWorkbench.tree") }}</strong>
      <button
        type="button"
        class="tree-import"
        :disabled="importing || loading"
        :title="t('studio.assetWorkbench.importFile')"
        :aria-label="t('studio.assetWorkbench.importFile')"
        @click="importFile"
      >
        <FIcon :name="importing ? 'Loader2' : 'FilePlus'" :size="13" aria-label="" />
      </button>
    </header>
    <p v-if="error" class="tree-error" role="alert">{{ error }}</p>
    <p v-else-if="loading && !rows.length" class="tree-empty">
      {{ t("common.loading") }}
    </p>
    <p v-else-if="!rows.length" class="tree-empty">
      {{ t("studio.assetWorkbench.treeEmpty") }}
    </p>
    <button
      v-for="row in rows"
      :key="row.relativePath"
      type="button"
      class="tree-row"
      :class="{
        folder: row.isFolder,
        active:
          !row.isFolder &&
          (row.relativePath === activeAsset || activeAsset.endsWith(`/${row.relativePath}`) || activeAsset.endsWith(`\\${row.relativePath}`)),
      }"
      :style="{ paddingInlineStart: `${8 + row.depth * 13}px` }"
      :title="row.relativePath"
      @click="onRowClick(row)"
    >
      <FIcon
        v-if="row.isFolder"
        :name="expanded.has(row.relativePath) ? 'FolderOpen' : 'Folder'"
        :size="13"
        aria-label=""
      />
      <FIcon v-else :name="fileIcon(row.name)" :size="13" aria-label="" />
      <span class="tree-name">{{ row.name }}</span>
      <span v-if="row.size !== null && !row.isFolder" class="tree-size">
        {{ row.size > 1048576 ? `${(row.size / 1048576).toFixed(1)}M` : `${Math.ceil(row.size / 1024)}K` }}
      </span>
    </button>
  </div>
</template>

<style scoped>
.asset-tree {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: auto;
}
.tree-head {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  gap: 6px;
  padding: 8px 10px;
}
.tree-head strong {
  flex: 1;
  font-size: 11px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--muted-foreground);
}
.tree-import {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 24px;
  min-width: 24px;
}
.tree-import:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--foreground);
}
.tree-import:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.tree-error {
  color: var(--danger);
  font-size: 11px;
  margin: 0;
  padding: 10px;
  overflow-wrap: anywhere;
}
.tree-empty {
  color: var(--muted-foreground);
  font-size: 11.5px;
  margin: 0;
  padding: 18px 10px;
  text-align: center;
}
.tree-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  gap: 6px;
  margin: 1px 6px;
  min-height: 26px;
  padding-block: 2px;
  padding-inline-end: 8px;
  text-align: start;
  width: calc(100% - 12px);
}
.tree-row:hover {
  background: var(--surface-hover);
}
.tree-row.folder {
  color: var(--muted-foreground);
  font-weight: 600;
}
.tree-row.active {
  background: color-mix(in srgb, var(--primary) 14%, transparent);
  color: var(--primary);
}
.tree-name {
  flex: 1;
  font-size: 11.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tree-size {
  color: var(--muted-foreground);
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}
</style>
