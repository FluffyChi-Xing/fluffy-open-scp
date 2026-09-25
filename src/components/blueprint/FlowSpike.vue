<script setup lang="ts">
/**
 * P0-3 spike：验证 @vue-flow/core 在本项目技术栈下的最小闭环——
 * 声明式节点/边、自定义节点模板、Handle 连线锚点是否可用。
 * 仅作为 happy-dom 兼容性验证与后续 BP0 的参照，不进入任何路由。
 */
import { VueFlow } from '@vue-flow/core';
import type { Edge, Node } from '@vue-flow/core';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';

const nodes: Node[] = [
  { id: 'spike-a', position: { x: 0, y: 0 }, data: { label: 'A' } },
  { id: 'spike-b', position: { x: 240, y: 120 }, data: { label: 'B' } },
];
const edges: Edge[] = [{ id: 'spike-e1', source: 'spike-a', target: 'spike-b' }];
</script>

<template>
  <div class="flow-spike" data-testid="flow-spike">
    <VueFlow :nodes="nodes" :edges="edges" :fit-view-on-init="false" />
  </div>
</template>

<style scoped>
.flow-spike {
  width: 640px;
  height: 480px;
}
</style>
