<script setup lang="ts">
import { SplitterPanel, type SplitterPanelProps } from "reka-ui";

defineProps<SplitterPanelProps>();
</script>

<template>
  <SplitterPanel class="resizable-panel" v-bind="$props">
    <slot />
  </SplitterPanel>
</template>

<style scoped>
.resizable-panel {
  min-height: 0;
  min-width: 0;
  overflow: hidden;
}
</style>
