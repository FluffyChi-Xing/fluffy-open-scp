<script setup lang="ts">
import FEmpty from "@/components/extensions/FEmpty.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import type { GameFolder } from "@/api/tauri";

interface Props {
  folders: GameFolder[];
  selectedPath: string;
  loading: boolean;
}
const props = defineProps<Props>();
const emit = defineEmits<{ select: [path: string] }>();
</script>

<template>
  <section class="folder-tree panel">
    <FTypography :header="4" spacing="none"
      ><FIcon name="FolderOpen" :size="15" aria-label="" />{{
        $t("package.fileTree")
      }}</FTypography
    >
    <div v-if="props.loading" class="loading-list">
      <FSkeleton v-for="index in 5" :key="index" height="30px" rounded />
    </div>
    <div v-else-if="props.folders.length" class="folder-list">
      <button
        v-for="folder in props.folders"
        :key="folder.path"
        class="folder-item"
        :class="{ active: folder.path === props.selectedPath }"
        type="button"
        @click="emit('select', folder.path)"
      >
        <FIcon name="FolderOpen" :size="16" aria-label="" /><span>{{
          folder.name
        }}</span
        ><small>{{ folder.packageCount }}</small>
      </button>
    </div>
    <FEmpty
      v-else
      icon-name="FolderOpen"
      :title="$t('package.noFolders')"
      :desc="$t('package.chooseFolderHint')"
      variant="compact"
    />
  </section>
</template>

<style scoped>
.panel {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  min-width: 0;
  padding: 16px;
}
.folder-tree h4 {
  align-items: center;
  display: flex;
  gap: 7px;
}
.folder-tree h4 svg {
  color: var(--primary);
}
.folder-list {
  display: grid;
  gap: 3px;
  margin-top: 14px;
}
.folder-item {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 9px;
  min-height: 36px;
  padding: 0 9px;
  text-align: start;
  width: 100%;
}
.folder-item:hover,
.folder-item.active {
  background: var(--accent);
  color: var(--foreground);
}
.folder-item.active {
  font-weight: 700;
}
.folder-item span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.folder-item small {
  color: var(--subtle-foreground);
  margin-inline-start: auto;
}
.loading-list {
  display: grid;
  gap: 8px;
  margin-top: 14px;
}
.folder-tree :deep(.f-empty) {
  min-height: 180px;
  width: 100%;
}
</style>
