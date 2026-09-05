<script setup lang="ts">
import FEmpty from "@/components/extensions/FEmpty.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import type { FTreeNode } from "@/components/extensions/tree";
import FTree from "@/components/extensions/FTree.vue";

interface Props {
  nodes: readonly FTreeNode[];
  title: string;
  emptyTitle: string;
}
const props = defineProps<Props>();
const emit = defineEmits<{ select: [keys: string[]] }>();
</script>

<template>
  <aside class="workspace-tree" :aria-label="props.title">
    <div class="tree-heading">
      <FIcon name="FolderOpen" :size="16" aria-label="" /><span>{{
        props.title
      }}</span>
    </div>
    <FTree
      v-if="props.nodes.length"
      :data="props.nodes"
      selectable
      default-expand-all
      @update:selected-keys="emit('select', $event)"
    />
    <FEmpty
      v-else
      icon-name="FolderOpen"
      :title="props.emptyTitle"
      variant="compact"
    />
  </aside>
</template>

<style scoped>
.workspace-tree {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  min-width: 0;
  padding: 16px;
}
.tree-heading {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 12px;
  font-weight: 750;
  gap: 7px;
  margin-bottom: 14px;
  text-transform: uppercase;
}
.workspace-tree :deep(.f-empty) {
  min-height: 170px;
  width: 100%;
}
</style>
