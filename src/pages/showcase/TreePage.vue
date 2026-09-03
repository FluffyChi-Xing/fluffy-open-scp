<script setup lang="ts">
import { ref } from 'vue'
import FTree from '@/components/extensions/FTree.vue'
import type { FTreeNode } from '@/components/extensions/tree'

const nodes: FTreeNode[] = [
  { key: 'workspace', label: 'workspace', icon: 'FolderOpen', children: [{ key: 'src', label: 'src', icon: 'FolderOpen', children: [{ key: 'main', label: 'main.ts', icon: 'FileText' }, { key: 'pages', label: 'pages', icon: 'FolderOpen' }] }, { key: 'package', label: 'package.json', icon: 'FileText' }] },
  { key: 'permissions', label: 'cms', icon: 'FolderOpen', children: [{ key: 'content-read', label: 'content:read', icon: 'FileText' }, { key: 'content-write', label: 'content:write', icon: 'FileText' }] }
]
const selectedKeys = ref<string[]>([])
const checkedKeys = ref<string[]>([])
</script>

<template><section class="page"><header><p class="eyebrow">{{ $t('showcase.eyebrow') }}</p><h1>Tree</h1><p>用于文件夹、CMS 结构和权限 token 的层级选择。</p></header><article class="panel"><FTree :data="nodes" checkable selectable select-all default-expand-all v-model:selected-keys="selectedKeys" v-model:checked-keys="checkedKeys" /><p>Selected: {{ selectedKeys.join(', ') || '—' }}</p><p>Checked: {{ checkedKeys.join(', ') || '—' }}</p></article></section></template>

<style scoped>.page{display:grid;gap:24px}.eyebrow{color:var(--primary);font-size:11px;font-weight:750;letter-spacing:.08em;margin:0 0 10px;text-transform:uppercase}h1{font-size:clamp(1.8rem,3vw,2.5rem);letter-spacing:-.045em;margin:0}header p:not(.eyebrow),.panel p{color:var(--muted-foreground);font-size:14px;margin:10px 0 0}.panel{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow-sm);padding:20px}</style>
