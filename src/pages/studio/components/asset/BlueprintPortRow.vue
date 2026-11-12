<script setup lang="ts">
import { Handle, Position } from "@vue-flow/core";

defineProps<{
  id?: string;
  type: "source" | "target";
  label: string;
}>();
</script>

<template>
  <div class="port-row" :class="{ output: type === 'source' }">
    <span>{{ label }}</span>
    <Handle
      :id="id"
      :type="type"
      :position="type === 'source' ? Position.Right : Position.Left"
      :aria-label="label"
    />
  </div>
</template>

<style scoped>
.port-row {
  align-items: center;
  color: var(--flow-text, #e6e6e6);
  display: flex;
  font-size: 10.5px;
  margin-inline: calc(0px - var(--blueprint-port-inset, 0px));
  min-height: 24px;
  overflow-wrap: anywhere;
  padding: 2px 11px;
  position: relative;
}
.port-row.output {
  justify-content: flex-end;
  text-align: end;
}
</style>
