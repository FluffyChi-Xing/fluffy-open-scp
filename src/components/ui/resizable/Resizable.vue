<script setup lang="ts">
import { SplitterGroup, type SplitterGroupProps } from "reka-ui";

withDefaults(defineProps<SplitterGroupProps>(), {
  keyboardResizeBy: 8,
});
</script>

<template>
  <SplitterGroup class="resizable-group" v-bind="$props">
    <slot />
  </SplitterGroup>
</template>

<style scoped>
.resizable-group {
  min-height: 0;
  min-width: 0;
}
</style>
