<script setup lang="ts">
import { computed } from "vue";
import {
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuPortal,
  ContextMenuRoot,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "reka-ui";
import type { WorkspaceEntry } from "@/api/tauri";

export interface TreeAction {
  type: "create-folder" | "create-markdown" | "rename" | "move" | "open";
  path: string;
  parent: string;
  isFile: boolean;
}

interface Props {
  entries: readonly WorkspaceEntry[];
  title: string;
  emptyHint: string;
  selectedPath?: string;
}
const props = defineProps<Props>();
const emit = defineEmits<{
  open: [path: string];
  action: [action: TreeAction];
}>();

interface TreeRow {
  key: string;
  type: "folder" | "file";
  path: string;
  label: string;
  depth: number;
  parent: string;
}

const rows = computed<TreeRow[]>(() => {
  const children = new Map<string, WorkspaceEntry[]>();
  for (const entry of props.entries) {
    const split = entry.relativePath.lastIndexOf("/");
    const parent = split === -1 ? "" : entry.relativePath.slice(0, split);
    const siblings = children.get(parent);
    if (siblings) siblings.push(entry);
    else children.set(parent, [entry]);
  }
  const result: TreeRow[] = [];
  const walk = (parent: string, depth: number) => {
    for (const entry of children.get(parent) ?? []) {
      const label = entry.relativePath.split("/").pop() ?? entry.relativePath;
      result.push({
        key: entry.relativePath,
        type: entry.kind,
        path: entry.relativePath,
        label,
        depth,
        parent,
      });
      if (entry.kind === "folder") walk(entry.relativePath, depth + 1);
    }
  };
  walk("", 0);
  return result;
});

function emitAction(type: TreeAction["type"], row: TreeRow) {
  emit("action", {
    type,
    path: row.path,
    parent: row.parent,
    isFile: row.type === "file",
  });
}
function openFile(row: TreeRow) {
  if (row.type === "file") emit("open", row.path);
}
</script>

<template>
  <aside
    class="workspace-tree"
    :aria-label="props.title"
    data-allow-context-menu
  >
    <p class="panel-label">{{ props.title }}</p>
    <ContextMenuRoot>
      <ContextMenuTrigger as-child>
        <div class="tree-canvas">
          <template v-if="rows.length">
            <ContextMenuRoot v-for="row in rows" :key="row.key">
              <ContextMenuTrigger as-child>
                <button
                  class="tree-row"
                  :class="[row.type, { selected: row.path === props.selectedPath }]"
                  :style="{ paddingInlineStart: `${row.depth * 14 + 10}px` }"
                  type="button"
                  @click="openFile(row)"
                >
                  <svg class="row-icon" viewBox="0 0 24 24" aria-hidden="true">
                    <path
                      v-if="row.type === 'folder'"
                      d="M4 7a2 2 0 0 1 2-2h3.6a2 2 0 0 1 1.6.8L12.4 7H18a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2Z"
                    />
                    <template v-else>
                      <path d="M6 3h8l4 4v14H6Z" />
                      <path d="M14 3v4h4" />
                    </template>
                  </svg>
                  <span class="row-name">{{ row.label }}</span>
                  <small v-if="row.type === 'folder'" class="row-kind">
                    {{ $t("workspace.folderKind") }}
                  </small>
                </button>
              </ContextMenuTrigger>
              <ContextMenuPortal>
                <ContextMenuContent class="ws-menu">
                  <template v-if="row.type === 'folder'">
                    <ContextMenuItem
                      class="ws-menu-item"
                      @select="emitAction('create-markdown', row)"
                    >
                      {{ $t("workspace.menuCreateMarkdown") }}
                    </ContextMenuItem>
                    <ContextMenuItem
                      class="ws-menu-item"
                      @select="emitAction('create-folder', row)"
                    >
                      {{ $t("workspace.menuCreateFolder") }}
                    </ContextMenuItem>
                    <ContextMenuSeparator class="ws-menu-separator" />
                  </template>
                  <ContextMenuItem
                    v-if="row.type === 'file'"
                    class="ws-menu-item"
                    @select="emitAction('open', row)"
                  >
                    {{ $t("workspace.menuOpen") }}
                  </ContextMenuItem>
                  <ContextMenuItem
                    class="ws-menu-item"
                    @select="emitAction('rename', row)"
                  >
                    {{ $t("workspace.menuRename") }}
                  </ContextMenuItem>
                  <ContextMenuItem
                    class="ws-menu-item"
                    @select="emitAction('move', row)"
                  >
                    {{ $t("workspace.menuMove") }}
                  </ContextMenuItem>
                </ContextMenuContent>
              </ContextMenuPortal>
            </ContextMenuRoot>
          </template>
          <p v-else class="tree-empty">{{ props.emptyHint }}</p>
        </div>
      </ContextMenuTrigger>
      <ContextMenuPortal>
        <ContextMenuContent class="ws-menu" :side-offset="4">
          <ContextMenuItem
            class="ws-menu-item"
            @select="
              emit('action', {
                type: 'create-folder',
                path: '',
                parent: '',
                isFile: false,
              })
            "
          >
            {{ $t("workspace.menuCreateFolderRoot") }}
          </ContextMenuItem>
          <ContextMenuItem
            class="ws-menu-item"
            @select="
              emit('action', {
                type: 'create-markdown',
                path: '',
                parent: '',
                isFile: false,
              })
            "
          >
            {{ $t("workspace.menuCreateMarkdownRoot") }}
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenuPortal>
    </ContextMenuRoot>
  </aside>
</template>

<style scoped>
.workspace-tree {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: auto;
}
.panel-label {
  color: var(--subtle-foreground);
  flex: none;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  margin: 0;
  padding: 12px 14px 6px;
  text-transform: uppercase;
}
.tree-canvas {
  flex: 1;
  min-height: 220px;
  padding-bottom: 10px;
}
.tree-row {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 7px;
  min-height: 30px;
  padding-inline-end: 10px;
  text-align: start;
  width: 100%;
}
.tree-row:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.tree-row.folder {
  color: var(--foreground);
  font-weight: 550;
}
.tree-row.selected {
  background: var(--accent);
  box-shadow: inset 3px 0 0 var(--primary);
  color: var(--foreground);
  font-weight: 700;
}
.row-icon {
  fill: none;
  flex: none;
  height: 14px;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
  width: 14px;
}
.row-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.row-kind {
  color: var(--subtle-foreground);
  font-size: 10px;
  margin-inline-start: auto;
}
.tree-empty {
  color: var(--subtle-foreground);
  font-size: 12px;
  padding: 26px 16px;
  text-align: center;
}
</style>

<style>
.ws-menu {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  min-width: 176px;
  padding: 5px;
  z-index: 60;
}
.ws-menu-item {
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  font-size: 12px;
  outline: none;
  padding: 7px 10px;
}
.ws-menu-item[data-highlighted] {
  background: var(--accent);
}
.ws-menu-separator {
  background: var(--border);
  height: 1px;
  margin: 5px 4px;
}
</style>
