<script setup lang="ts">
import { shallowRef } from "vue";
import type { GameFolder, PackageFile } from "@/api/tauri";
import { extensionIconUrl } from "@/lib/resource-types";
import GameFolderTreeNode from "./GameFolderTreeNode.vue";

interface Props {
  folder: GameFolder;
  depth: number;
  defaultExpanded?: boolean;
}
const props = withDefaults(defineProps<Props>(), { defaultExpanded: false });
const emit = defineEmits<{ open: [file: PackageFile] }>();
const expanded = shallowRef(props.defaultExpanded);

function isPackage(file: PackageFile) {
  return file.name.toLowerCase().endsWith(".package");
}
function toggle() {
  expanded.value = !expanded.value;
}
function openPackage(file: PackageFile) {
  if (isPackage(file)) emit("open", file);
}
</script>

<template>
  <div class="tree-node" role="treeitem" :aria-expanded="expanded">
    <button
      class="tree-row folder-row"
      :style="{ paddingInlineStart: `${props.depth * 14 + 10}px` }"
      type="button"
      @click="toggle"
    >
      <svg
        class="chevron"
        :class="{ open: expanded }"
        viewBox="0 0 24 24"
        aria-hidden="true"
      >
        <path d="m9 6 6 6-6 6" />
      </svg>
      <svg class="folder-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path
          d="M4 7a2 2 0 0 1 2-2h3.6a2 2 0 0 1 1.6.8L12.4 7H18a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2Z"
        />
      </svg>
      <span class="row-name">{{ folder.name }}</span>
      <small v-if="folder.packageCount" class="row-badge">{{
        folder.packageCount
      }}</small>
    </button>
    <template v-if="expanded">
      <GameFolderTreeNode
        v-for="child in folder.children"
        :key="child.path"
        :folder="child"
        :depth="props.depth + 1"
        @open="emit('open', $event)"
      />
      <button
        v-for="file in folder.files"
        :key="file.path"
        class="tree-row file-row"
        :class="{ package: isPackage(file) }"
        :style="{ paddingInlineStart: `${(props.depth + 1) * 14 + 10}px` }"
        :disabled="!isPackage(file)"
        :title="
          isPackage(file) ? file.path : `${file.name} · 仅 .package 可打开`
        "
        type="button"
        @click="openPackage(file)"
      >
        <img class="file-icon" :src="extensionIconUrl(file.name)" alt="" />
        <span class="row-name">{{ file.name }}</span>
      </button>
    </template>
  </div>
</template>

<style scoped>
.tree-row {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 6px;
  min-height: 27px;
  padding-inline-end: 10px;
  text-align: start;
  width: 100%;
}
.tree-row:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.chevron,
.folder-icon {
  fill: none;
  flex: none;
  height: 13px;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 13px;
}
.chevron {
  transition: transform 120ms ease;
}
.chevron.open {
  transform: rotate(90deg);
}
.folder-icon {
  color: var(--muted-foreground);
}
.folder-row {
  color: var(--foreground);
  font-weight: 550;
}
.folder-row:hover .folder-icon {
  color: var(--primary);
}
.row-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.row-badge {
  background: var(--surface-hover);
  border-radius: 999px;
  color: var(--subtle-foreground);
  flex: none;
  font-size: 10px;
  margin-inline-start: auto;
  padding: 0 6px;
}
.file-row {
  color: var(--muted-foreground);
  font-weight: 400;
}
.file-row.package {
  cursor: pointer;
}
.file-row.package:hover {
  color: var(--primary);
}
.file-row:disabled {
  cursor: default;
  opacity: 0.55;
}
.file-row:disabled:hover {
  background: transparent;
  color: var(--muted-foreground);
}
.file-icon {
  flex: none;
  height: 14px;
  object-fit: contain;
  width: 14px;
}
</style>
