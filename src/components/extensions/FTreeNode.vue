<script setup lang="ts">
import { ChevronRight } from 'lucide-vue-next'
import { Checkbox } from '@/components/ui/checkbox'
import FIcon from './FIcon.vue'
import type { FTreeNode } from './tree'
import { checkState } from './tree'

interface Props {
  node: FTreeNode
  level: number
  selectedKeys: readonly string[]
  checkedKeys: readonly string[]
  expandedKeys: readonly string[]
  checkable: boolean
  selectable: boolean
}
const props = defineProps<Props>()
const emit = defineEmits<{ select: [key: string]; check: [key: string, checked: boolean]; toggle: [key: string] }>()
const children = props.node.children ?? []
const isExpanded = () => props.expandedKeys.includes(props.node.key)
const isSelected = () => props.selectedKeys.includes(props.node.key)
const state = () => checkState(props.node, new Set(props.checkedKeys))
function onCheck(value: boolean | 'indeterminate') { emit('check', props.node.key, value === true) }
function forwardCheck(key: string, checked: boolean) { emit('check', key, checked) }
</script>

<template>
  <li role="none">
    <div class="tree-row" :class="{ selected: isSelected(), disabled: props.node.disabled }" :style="{ paddingInlineStart: `${props.level * 18}px` }" role="treeitem" :aria-level="props.level + 1" :aria-expanded="children.length ? isExpanded() : undefined" :aria-selected="props.selectable ? isSelected() : undefined">
      <button v-if="children.length" class="tree-expander" type="button" :aria-label="isExpanded() ? 'Collapse' : 'Expand'" @click="emit('toggle', props.node.key)"><ChevronRight :class="{ expanded: isExpanded() }" :size="16" /></button>
      <span v-else class="tree-expander-placeholder" />
      <Checkbox v-if="props.checkable && props.node.checkable !== false" :model-value="state().indeterminate ? 'indeterminate' : state().checked" :disabled="props.node.disabled" @update:model-value="onCheck" />
      <FIcon v-if="props.node.icon" :name="props.node.icon" size="16" />
      <button class="tree-label" type="button" :disabled="props.node.disabled || props.node.selectable === false" @click="emit('select', props.node.key)">{{ props.node.label }}</button>
    </div>
    <ul v-if="children.length && isExpanded()" role="group" class="tree-children"><FTreeNode v-for="child in children" :key="child.key" :node="child" :level="props.level + 1" :selected-keys="props.selectedKeys" :checked-keys="props.checkedKeys" :expanded-keys="props.expandedKeys" :checkable="props.checkable" :selectable="props.selectable" @select="emit('select', $event)" @check="forwardCheck" @toggle="emit('toggle', $event)" /></ul>
  </li>
</template>

<style scoped>.tree-row{align-items:center;border-radius:var(--radius-sm);display:flex;gap:6px;min-height:32px}.tree-row:hover:not(.disabled),.tree-row.selected{background:var(--accent)}.tree-row.disabled{opacity:.55}.tree-expander,.tree-expander-placeholder{align-items:center;background:none;border:0;color:var(--muted-foreground);display:inline-flex;flex:none;height:22px;justify-content:center;padding:0;width:22px}.tree-expander{cursor:pointer}.tree-expander svg{transition:transform .15s}.tree-expander svg.expanded{transform:rotate(90deg)}.tree-label{background:none;border:0;color:inherit;cursor:pointer;flex:1;overflow:hidden;padding:4px;text-align:start;text-overflow:ellipsis;white-space:nowrap}.tree-label:disabled{cursor:not-allowed}.tree-children{list-style:none;margin:0;padding:0}</style>
