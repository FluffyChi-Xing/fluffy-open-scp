<script setup lang="ts">
import FSkeleton from "@/components/ui/FSkeleton.vue";
import type { GameFolder, PackageFile } from "@/api/tauri";
import GameFolderTreeNode from "./GameFolderTreeNode.vue";

interface Props {
  folders: GameFolder[];
  loading: boolean;
}
const props = defineProps<Props>();
const emit = defineEmits<{ open: [file: PackageFile] }>();
</script>

<template>
  <section class="folder-tree">
    <p class="panel-label">{{ $t("package.fileTree") }}</p>
    <div v-if="props.loading" class="tree-loading">
      <FSkeleton v-for="index in 6" :key="index" height="22px" rounded />
    </div>
    <div v-else-if="props.folders.length" class="tree-body" role="tree">
      <GameFolderTreeNode
        v-for="folder in props.folders"
        :key="folder.path"
        :folder="folder"
        :depth="0"
        :default-expanded="true"
        @open="emit('open', $event)"
      />
    </div>
    <div v-else class="tree-empty">
      <p>{{ $t("package.noFolders") }}</p>
      <small>{{ $t("package.chooseFolderHint") }}</small>
    </div>
  </section>
</template>

<style scoped>
.folder-tree {
  background: var(--surface);
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: auto;
}
.panel-label {
  background: var(--surface);
  color: var(--subtle-foreground);
  flex: none;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  margin: 0;
  padding: 10px 12px 6px;
  position: sticky;
  text-transform: uppercase;
  top: 0;
  z-index: 1;
}
.tree-loading {
  display: grid;
  gap: 6px;
  padding: 8px 12px;
}
.tree-body {
  padding-bottom: 10px;
}
.tree-empty {
  color: var(--subtle-foreground);
  display: grid;
  gap: 4px;
  padding: 28px 16px;
  text-align: center;
}
.tree-empty p {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
}
.tree-empty small {
  font-size: 11px;
  line-height: 1.5;
}
</style>
