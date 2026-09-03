<script setup lang="ts">
import { computed, shallowRef } from 'vue'
import { Checkbox } from '@/components/ui/checkbox'
import FTreeNode from './FTreeNode.vue'
import type { FTreeNode as TreeNode } from './tree'
import { flattenTree, toggleChecked } from './tree'

interface Props { data: readonly TreeNode[]; checkable?: boolean; selectable?: boolean; selectionMode?: 'single' | 'multiple'; checkStrictly?: boolean; selectAll?: boolean; defaultExpandAll?: boolean }
const props = withDefaults(defineProps<Props>(), { selectionMode: 'single' })
const selectedKeys = defineModel<string[]>('selectedKeys', { default: () => [] })
const checkedKeys = defineModel<string[]>('checkedKeys', { default: () => [] })
const expandedKeys = defineModel<string[]>('expandedKeys', { default: () => [] })
const initialized = shallowRef(false)
if (props.defaultExpandAll && !initialized.value) { expandedKeys.value = flattenTree(props.data).filter((node) => node.children?.length).map((node) => node.key); initialized.value = true }
const checkableNodes = computed(() => flattenTree(props.data).filter((node) => !node.disabled && node.checkable !== false))
const allChecked = computed(() => checkableNodes.value.length > 0 && checkableNodes.value.every((node) => checkedKeys.value.includes(node.key)))
const partiallyChecked = computed(() => !allChecked.value && checkableNodes.value.some((node) => checkedKeys.value.includes(node.key)))
function select(key: string) { selectedKeys.value = props.selectionMode === 'single' ? [key] : selectedKeys.value.includes(key) ? selectedKeys.value.filter((item) => item !== key) : [...selectedKeys.value, key] }
function check(key: string, checked: boolean) { checkedKeys.value = toggleChecked(props.data, checkedKeys.value, key, checked, props.checkStrictly) }
function toggle(key: string) { expandedKeys.value = expandedKeys.value.includes(key) ? expandedKeys.value.filter((item) => item !== key) : [...expandedKeys.value, key] }
function toggleAll(value: boolean | 'indeterminate') { checkedKeys.value = value === true ? checkableNodes.value.map((node) => node.key) : [] }
</script>

<template>
  <section class="f-tree">
    <label v-if="props.checkable && props.selectAll" class="tree-select-all"><Checkbox :model-value="partiallyChecked ? 'indeterminate' : allChecked" @update:model-value="toggleAll" />全选</label>
    <ul role="tree" class="tree-root"><FTreeNode v-for="node in props.data" :key="node.key" :node="node" :level="0" :selected-keys="selectedKeys" :checked-keys="checkedKeys" :expanded-keys="expandedKeys" :checkable="props.checkable" :selectable="props.selectable" @select="select" @check="check" @toggle="toggle" /></ul>
  </section>
</template>

<style scoped>.f-tree{color:var(--foreground);font-size:14px}.tree-root{list-style:none;margin:0;padding:0}.tree-select-all{align-items:center;color:var(--muted-foreground);display:flex;font-size:13px;gap:8px;margin-bottom:8px}</style>
