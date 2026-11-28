<script setup lang="ts">
import { ref, watch } from "vue";
import { ChevronRight, ChevronDown } from "lucide-vue-next";
import SidebarMenuSub from "@/components/ui/sidebar/SidebarMenuSub.vue";
import SidebarMenuSubItem from "@/components/ui/sidebar/SidebarMenuSubItem.vue";
import SidebarMenuSubButton from "@/components/ui/sidebar/SidebarMenuSubButton.vue";
import { useI18n } from "vue-i18n";
import type { DomNode } from "./elements";
const props = defineProps<{
  node: DomNode;
  children: Map<number, DomNode[]>;
  selected?: number;
  expanded: Set<number>;
}>();
const emit = defineEmits<{
  select: [node: DomNode];
  hover: [node?: DomNode];
}>();
const { t } = useI18n();
const open = ref(props.expanded.has(props.node.i));
watch(
  () => props.expanded,
  (expanded) => {
    if (expanded.has(props.node.i)) open.value = true;
  },
);
</script>
<template>
  <SidebarMenuSubItem>
    <div
      tabindex="0"
      class="element-row"
      :data-node-id="node.i"
      :data-active="selected === node.i"
      :aria-expanded="children.has(node.i) ? open : undefined"
      @click="
        emit('select', node);
        if (children.has(node.i)) open = !open;
      "
      @keydown.right.prevent="open = true"
      @keydown.left.prevent="open = false"
      @mouseenter="emit('hover', node)"
      @mouseleave="emit('hover')"
    >
      <button
        v-if="children.has(node.i)"
        type="button"
        class="branch"
        :aria-label="t(open ? 'devtool.collapse' : 'devtool.expand')"
        :aria-expanded="open"
        @click.stop="open = !open"
        @keydown.stop
      >
        <component
          :is="open ? ChevronDown : ChevronRight"
          v-if="children.has(node.i)"
          :size="13"
        />
      </button>
      <span v-else class="branch-placeholder" />
      <SidebarMenuSubButton
        as="button"
        class="node-label"
        @click.stop="
          emit('select', node);
          if (children.has(node.i)) open = !open;
        "
      >
        <span class="tag">&lt;{{ node.tag }}</span
        ><span v-if="node.id"> id="{{ node.id }}"</span>
        <span v-if="node.classes" class="attribute">
          class="{{ node.classes }}"</span
        ><span class="tag">&gt;</span>
        <span class="node-text">{{ node.text }}</span>
      </SidebarMenuSubButton>
    </div>
    <SidebarMenuSub
      v-if="open && children.has(node.i)"
      class="element-children"
    >
      <ElementTreeNode
        v-for="child in children.get(node.i)"
        :key="child.i"
        :node="child"
        :children="props.children"
        :selected="selected"
        :expanded="expanded"
        @select="emit('select', $event)"
        @hover="emit('hover', $event)"
      />
    </SidebarMenuSub>
  </SidebarMenuSubItem>
</template>
<style scoped>
.element-row {
  display: flex;
  align-items: center;
  cursor: pointer;
  width: 100%;
  justify-content: flex-start;
  white-space: nowrap;
  text-align: left;
  font-family: var(--font-mono, monospace);
  font-size: 12px;
  border-radius: 3px;
  color: var(--foreground);
  min-height: 28px;
  padding: 3px 6px;
  gap: 2px;
}
.element-row:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: -2px;
}
.element-row:hover {
  background: var(--surface-hover);
}
.element-row[data-active="true"] {
  background: var(--sidebar-active);
  color: var(--sidebar-active-foreground);
}
.node-label {
  flex: 1;
  min-width: 0;
  padding: 0;
  font: inherit;
  color: inherit;
  background: transparent;
  border: 0;
  text-align: left;
}
.branch-placeholder {
  width: 24px;
  flex-shrink: 0;
}
.branch {
  width: 24px;
  height: 28px;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: 0;
  color: inherit;
  cursor: pointer;
  padding: 0;
  flex-shrink: 0;
  display: inline-flex;
}
.branch > svg {
  pointer-events: none;
}
.tag {
  color: var(--primary);
}
.attribute {
  color: var(--warning);
}
.node-text {
  color: var(--muted-foreground);
}
.element-children {
  margin: 0 0 0 14px;
  padding: 0 0 0 7px;
  border-color: var(--border);
}
</style>
