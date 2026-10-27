<script setup lang="ts">
import { computed, ref } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";

/**
 * 菜单复制目标的树状选择器：一级菜单为根节点，带子菜单容器的建筑
 * （消防局/大学/小学…）作为可展开的子节点 —— 选中建筑节点即以该建筑的
 * 子菜单容器为复制目标（游戏内条目将出现在建筑的面板里）。
 * 样式全部走项目 design token。
 */
export interface CopyTargetNode {
  /** 目标 id：一级菜单为分类 instance；子菜单节点为容器 instance。 */
  id: string;
  label: string;
  kind: "menu" | "container";
  children?: CopyTargetNode[];
}

const props = defineProps<{ nodes: CopyTargetNode[]; placeholder?: string }>();
const selected = defineModel<{ id: string; label: string; kind: string } | null>({ default: null });

const open = ref(false);
const expanded = ref<Set<string>>(new Set());

const triggerText = computed(() => selected.value?.label ?? props.placeholder ?? "");

function toggleExpand(id: string): void {
  const next = new Set(expanded.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  expanded.value = next;
}

function choose(node: CopyTargetNode): void {
  selected.value = { id: node.id, label: node.label, kind: node.kind };
  open.value = false;
}

function closeOnBackdrop(): void {
  open.value = false;
}
</script>

<template>
  <div class="tree-anchor">
    <button
      type="button"
      class="tree-trigger"
      :aria-expanded="open"
      aria-haspopup="tree"
      @click="open = !open"
    >
      <span class="tree-trigger-text" :class="{ placeholder: !selected }">{{ triggerText }}</span>
      <FIcon name="ChevronDown" :size="13" aria-label="" />
    </button>

    <div v-if="open" class="tree-backdrop" @click="closeOnBackdrop"></div>
    <div v-if="open" class="tree-panel" role="tree">
      <template v-for="root in nodes" :key="root.id">
        <div class="tree-row" :class="{ selected: selected?.id === root.id }">
          <button
            v-if="root.children && root.children.length > 0"
            type="button"
            class="tree-expander"
            :aria-label="expanded.has(root.id) ? 'collapse' : 'expand'"
            @click.stop="toggleExpand(root.id)"
          >
            <FIcon
              :name="expanded.has(root.id) ? 'ChevronDown' : 'ChevronRight'"
              :size="12"
              aria-label=""
            />
          </button>
          <span v-else class="tree-expander-spacer"></span>
          <button type="button" class="tree-item" @click="choose(root)">
            {{ root.label }}
          </button>
        </div>
        <template v-if="root.children && expanded.has(root.id)">
          <div
            v-for="child in root.children"
            :key="child.id"
            class="tree-row child"
            :class="{ selected: selected?.id === child.id }"
          >
            <span class="tree-expander-spacer"></span>
            <button type="button" class="tree-item" @click="choose(child)">
              <FIcon name="Building2" :size="12" aria-label="" />
              {{ child.label }}
            </button>
          </div>
        </template>
      </template>
    </div>
  </div>
</template>

<style scoped>
.tree-anchor {
  position: relative;
  width: 100%;
  min-width: 0;
}
.tree-trigger {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 13px;
  gap: 8px;
  height: 32px;
  justify-content: space-between;
  min-width: 0;
  padding: 0 10px;
  width: 100%;
}
.tree-trigger:hover {
  border-color: var(--border-strong);
}
.tree-trigger-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tree-trigger-text.placeholder {
  color: var(--muted-foreground);
}
.tree-backdrop {
  bottom: 0;
  left: 0;
  position: fixed;
  right: 0;
  top: 0;
  z-index: 30;
}
.tree-panel {
  background: var(--surface-elevated, var(--surface));
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: 0 10px 28px rgb(0 0 0 / 35%);
  max-height: 260px;
  overflow: auto;
  padding: 6px;
  position: absolute;
  top: calc(100% + 4px);
  width: 100%;
  z-index: 31;
}
.tree-row {
  align-items: center;
  border-radius: var(--radius-sm);
  display: flex;
  gap: 2px;
}
.tree-row.selected .tree-item {
  color: var(--primary);
  font-weight: 600;
}
.tree-row.child .tree-item {
  font-size: 12px;
}
.tree-expander {
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  flex: none;
  padding: 2px;
}
.tree-expander-spacer {
  flex: none;
  width: 16px;
}
.tree-item {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 13px;
  gap: 6px;
  min-width: 0;
  padding: 5px 8px;
  text-align: start;
  width: 100%;
}
.tree-item:hover {
  background: var(--surface-hover);
}
</style>
